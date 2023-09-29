use crate::utils::RECIPROCAL_U32;

/// A trait that defines the interface for making predictions.
pub trait Model {
    /// Construct a new model.
    fn new() -> Self;

    /// Return a probability prediction in the 16-bit range using the
    /// internal state.
    #[must_use]
    fn predict(&self) -> u16;

    /// Update the internal context with the next bit 'bit'.
    fn update(&mut self, bit: u8);
}

/// A simple model that predicts the probability of the next bit.
/// CONTEXT_SIZE_BITS defines the size of the cache (history).
/// LIMIT defines the maximum number of samples for bucket.
pub struct BitwiseModel<const CONTEXT_SIZE_BITS: usize, const LIMIT: usize> {
    ctx: u64,
    cache: Vec<(u16, u16)>,
}

impl<const CTX_SIZE_BITS: usize, const LIMIT: usize> Model
    for BitwiseModel<CTX_SIZE_BITS, LIMIT>
{
    fn new() -> Self {
        Self {
            ctx: 0,
            cache: vec![(1, 1); 1 << CTX_SIZE_BITS],
        }
    }

    fn predict(&self) -> u16 {
        // Return a probability prediction in the 16-bit range using the
        // 'CTX_SIZE_BITS' LSB bits in 'ctx'.
        let key = self.ctx % (1 << CTX_SIZE_BITS);
        let (set, cnt) = self.cache[key as usize];
        debug_assert!(cnt < 1024);
        let a = set as u64;
        let b = 1 + cnt as u64;

        // This is equivalent to (a * (1<<16)) / b;
        ((a * (RECIPROCAL_U32[b as usize] as u64)) >> 16) as u16
    }

    fn update(&mut self, bit: u8) {
        // Update the probability of the context 'ctx', considering the first
        // 'CTX_SIZE_BITS' LSB bits, with the bit 'bit'.
        let key = self.ctx % (1 << CTX_SIZE_BITS);
        let (set, cnt) = &mut self.cache[key as usize];
        *cnt += 1;
        *set += (bit & 1) as u16;
        // Normalize the count if LIMIT is exceeded. This allows new data to
        // have a higher weight.
        if *cnt as usize >= LIMIT {
            *set /= 2;
            *cnt /= 2;
        }
        // Update the context.
        self.ctx = (self.ctx << 1) + bit as u64;
    }
}

#[test]
fn test_simple_model() {
    {
        let mut model = BitwiseModel::<7, 1024>::new();
        for _ in 0..10000 {
            model.update(1);
            model.update(0);
        }

        // Predict a '1'
        let pred = model.predict();
        assert!(pred > 64_000);
        model.update(1);

        // Predict a zero.
        let pred = model.predict();
        assert!(pred < 1_000);
    }

    {
        let mut model = BitwiseModel::<7, 256>::new();
        for _ in 0..10000 {
            model.update(0);
        }
        // The prediction needs to be close to zero.
        let pred = model.predict();
        assert_eq!(pred, 0);
    }

    {
        let mut model = BitwiseModel::<7, 256>::new();
        for _ in 0..10000 {
            model.update(1);
        }
        // The prediction needs to be close to one.
        let pred = model.predict();
        assert!(pred > 65_000);
    }
}

/// Start with context of n bits.
const DMC_LEVELS: usize = 3;

/// If the number of states reaches this number, reset the model.
const DMC_MAX_NODES: usize = 10_000_000;

pub struct DMCModel {
    state: usize,
    /// Maps the current state to the next (0, 1) states.
    states: Vec<[u32; 2]>,
    /// Records the counts (events seen) for each edge.
    counts: Vec<[u16; 2]>,
}

impl DMCModel {
    /// Create the initial state machine that has a tree-structure with 'layers'
    fn init(&mut self, layers: usize) {
        assert_eq!(self.states.len(), 0);
        assert_eq!(self.counts.len(), 0);
        assert_eq!(self.state, 0);
        let _ = self.allocate_new_state([0, 0], [0, 0]);
        for layer in 1..layers {
            let len = (1 << layer) - 1;
            for _ in 0..len {
                let left = self.allocate_new_state([0, 0], [0, 0]);
                let right = self.allocate_new_state([0, 0], [0, 0]);
                self.states[(left / 2) as usize][0] = left;
                self.states[(left / 2) as usize][1] = right;
            }
        }
    }

    /// Allocate a new state and return it's index.
    fn allocate_new_state(&mut self, next: [u32; 2], counts: [u16; 2]) -> u32 {
        self.states.push(next);
        self.counts.push(counts);
        (self.counts.len() - 1) as u32
    }

    fn verify(&self) {
        if cfg!(debug_assertions) {
            debug_assert_eq!(self.counts.len(), self.states.len());
            let len = self.counts.len();
            for i in 0..len {
                debug_assert!(
                    (self.states[i][0] as usize) < len
                        && (self.states[i][1] as usize) < len
                );
            }
        }
    }

    pub fn reset(&mut self) {
        self.state = 0;
        self.states.clear();
        self.counts.clear();
        self.init(DMC_LEVELS);
    }

    pub fn try_clone(&mut self, edge: usize) {
        if self.states.len() > DMC_MAX_NODES {
            self.reset();
            return;
        }
        let curr = self.state;
        let from = curr;
        let to = self.states[curr][edge] as usize;

        // This is the cost of the edge that we want to redirect.
        let edge_count = self.counts[from][edge] as u64;
        let sum = self.counts[to][0] as u64 + self.counts[to][1] as u64;

        // Don't clone edges that are too weak, or don't contribute much to the
        // sum node.
        if edge_count < 16 || sum < edge_count * 2 {
            return;
        }

        debug_assert!(edge_count != 0);
        debug_assert!(sum != 0);
        debug_assert!(edge_count != sum);

        // Create a new node.
        let tc = self.counts[to];
        let r = edge_count / sum;
        assert!(r != 1);
        let tc0 = ((tc[0] as u64 * edge_count) / sum) as u16;
        let tc1 = ((tc[1] as u64 * edge_count) / sum) as u16;
        self.counts[to][0] -= tc0;
        self.counts[to][1] -= tc1;
        let new = self.allocate_new_state(self.states[to], [tc0, tc1]);
        // Register the new node.
        self.states[curr][edge] = new;
        self.verify();
    }

    /// Print a dotty graph of the state machine.
    pub fn dump(&self) {
        if cfg!(debug_assertions) {
            println!("digraph finite_state_machine {{");
            println!("rankdir=LR;");
            println!("node [shape = circle];");
            for i in 0..self.counts.len() {
                let tos = self.states[i];
                let counts = self.counts[i];
                println!("{} -> {} [label = \"0: {}\"];", i, tos[0], counts[0]);
                println!("{} -> {} [label = \"1: {}\"];", i, tos[1], counts[1]);
            }
            println!("}}");
        }
    }
}

impl Model for DMCModel {
    fn new() -> Self {
        let mut model = DMCModel {
            state: 0,
            states: Vec::new(),
            counts: Vec::new(),
        };
        model.init(DMC_LEVELS);
        model
    }

    /// Return a probability prediction in the 16-bit range.
    fn predict(&self) -> u16 {
        self.verify();
        let counts = self.counts[self.state];
        let a = counts[1] as u64;
        let b = counts[0] as u64 + a;
        if b == 0 {
            return 1 << 15;
        }
        ((a * 65535) / b) as u16
    }

    /// Update the probability of the model with the bit 'bit'.
    /// Advance to the next state, and update the counts.
    fn update(&mut self, bit: u8) {
        self.try_clone(bit as usize);
        self.counts[self.state][bit as usize] += 1;
        self.state = self.states[self.state][bit as usize] as usize;
        self.verify();
    }
}

#[test]
fn test_dmc_dump() {
    let text = "this is a message. this is a message.  this is a message.";
    let text = text.as_bytes();
    let mut model = DMCModel::new();

    for b in text {
        for i in 0..8 {
            let bit = (b >> i) & 1;
            let p = model.predict();
            model.update(bit);
            println!("pred = {}", p);
        }
    }
    model.dump();
}

#[test]
fn dmc_zeros() {
    let mut model = DMCModel::new();

    for _ in 0..(1 << 13) {
        model.update(0);
    }

    let p = model.predict();
    model.update(0);

    // Very high probability that this is a zero.
    assert!(p < 10);
}

#[test]
fn dmc_pattern() {
    let mut model = DMCModel::new();

    // Train a pattern.
    for _ in 0..2000 {
        model.update(0);
        model.update(1);
        model.update(1);
        model.update(0);
    }

    // Check that we can predict it.
    let p1 = model.predict();
    model.update(0);
    let p2 = model.predict();
    model.update(1);
    let p3 = model.predict();
    model.update(1);
    let p4 = model.predict();
    model.update(0);

    // Detect the pattern 0110.
    assert!(p1 < 10);
    assert!(p2 > 65_000);
    assert!(p3 > 65_000);
    assert!(p4 < 10);
}
