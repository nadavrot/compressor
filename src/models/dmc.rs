//! This module implements the Dynamic Markov compression, that uses a state
//! machine to predict the next bit in the sequence. The algorithm is explained
//! very well in the book Managing Gigabytes by Witten, Moffat and Bell,
//! section 2.5, page 70.

use super::Model;

/// Start with context of n bits.
const DMC_LEVELS: usize = 16;

/// If the number of states reaches this number, reset the model.
const DMC_MAX_NODES: usize = 10_000_000;

/// Represents a node in the DMC state machine.
#[derive(Clone)]
pub struct DMCNode {
    /// Points to the next nodes (left - 0, right - 1).
    pub next: [u32; 2],
    /// Represents the counts on each edge (left - 0, right - 1).
    pub counts: [u16; 2],
}

impl DMCNode {
    /// Create a new empty node.
    pub fn empty() -> Self {
        Self {
            next: [0, 0],
            counts: [0, 0],
        }
    }
}

/// This struct represents a state machine where each transition between state
/// is a one or a zero that represents a bit in the input stream. The counts on
/// the edges represent the probability of the next bit being one or zero.
pub struct DMCModel {
    /// The current state.
    state: usize,
    /// The list of states.
    nodes: Vec<DMCNode>,
}

impl DMCModel {
    /// Create the initial state machine that has a cycle with 'num' elements
    /// in the loop. This value should be a multiple of 8.
    fn init(&mut self, num: usize) {
        assert_eq!(self.nodes.len(), 0);
        assert_eq!(self.state, 0);
        self.nodes = vec![DMCNode::empty(); num];

        // Create a cycle with 'num' elements. Each element points to the next
        // one with both edges. A->B->C ... ->A.
        //
        // This is described in:
        // DATA COMPRESSION USING DYNAMIC MARKOV MODELLING; Cormack, Horspool.
        // Page 8, section 4.3.
        for i in 0..num {
            let next = [((i + 1) % num) as u32, ((i + 1) % num) as u32];
            self.nodes[i].next = next;
        }
        self.verify();
    }

    /// Allocate a new state and return it's index.
    fn add_state(&mut self, node: DMCNode) -> u32 {
        self.nodes.push(node);
        (self.nodes.len() - 1) as u32
    }

    fn verify(&self) {
        if cfg!(debug_assertions) {
            debug_assert!(self.state < self.nodes.len());
            let len = self.nodes.len();
            for i in 0..len {
                debug_assert!(
                    (self.nodes[i].next[0] as usize) < len
                        && (self.nodes[i].next[1] as usize) < len
                );
            }
        }
    }

    pub fn reset(&mut self) {
        self.state = 0;
        self.nodes.clear();
        self.init(DMC_LEVELS);
    }

    pub fn try_clone(&mut self, edge: usize) {
        if self.nodes.len() > DMC_MAX_NODES {
            self.reset();
            return;
        }
        let curr = self.state;
        let from = curr;
        let to = self.nodes[curr].next[edge] as usize;

        // This is the cost of the edge that we want to redirect.
        let edge_count = self.nodes[from].counts[edge] as u64;
        let to_node = &mut self.nodes[to];
        let sum = to_node.counts[0] as u64 + to_node.counts[1] as u64;

        // Don't clone edges that are too weak, or don't contribute much to the
        // sum node.
        if edge_count < 4 || sum < edge_count * 2 {
            return;
        }

        debug_assert!(edge_count != 0);
        debug_assert!(sum != 0);
        debug_assert!(edge_count != sum);

        // Create a new node.
        let tc = to_node.counts;
        let tc0 = ((tc[0] as u64 * edge_count) / sum) as u16;
        let tc1 = ((tc[1] as u64 * edge_count) / sum) as u16;
        to_node.counts[0] -= tc0;
        to_node.counts[1] -= tc1;
        let mut node = DMCNode::empty();
        node.counts = [tc0, tc1];
        node.next = to_node.next;
        // Register the new node.
        self.nodes[curr].next[edge] = self.add_state(node);
        self.verify();
    }

    /// Print a dotty graph of the state machine.
    pub fn dump(&self) {
        if cfg!(debug_assertions) {
            println!("digraph finite_state_machine {{");
            println!("rankdir=LR;");
            println!("node [shape = circle];");
            for i in 0..self.nodes.len() {
                let tos = self.nodes[i].next;
                let counts = self.nodes[i].counts;
                println!("{} -> {} [label = \"0) {}\"];", i, tos[0], counts[0]);
                println!("{} -> {} [label = \"1) {}\"];", i, tos[1], counts[1]);
            }
            println!("}}");
        }
    }
}

impl Model for DMCModel {
    fn new() -> Self {
        let mut model = DMCModel {
            state: 0,
            nodes: Vec::new(),
        };
        model.init(DMC_LEVELS);
        model
    }

    /// Return a probability prediction in the 16-bit range.
    fn predict(&self) -> u16 {
        let counts = self.nodes[self.state].counts;
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
        self.nodes[self.state].counts[bit as usize] += 1;
        self.state = self.nodes[self.state].next[bit as usize] as usize;
    }
}

#[test]
fn test_dmc_dump() {
    let text = "this is a message. this is a message. this is a message.";
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
    assert!(p1 < 40);
    assert!(p2 > 65_000);
    assert!(p3 > 65_000);
    assert!(p4 < 40);
}
