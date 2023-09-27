//! This module implements an arithmetic code such as the one in zpaq, and
//! described by Matt Mahoney: https://mattmahoney.net/dc/dce.html#Section_32
//! and in the book Managing Gigabytes by Witten, Moffat and Bell, section 2.4.
use rand_distr::num_traits::Zero;

use crate::utils::signatures::{match_signature, ARITH_SIG};
use crate::utils::signatures::{read32, write32};
use crate::utils::RECIPROCAL_U32;
use crate::{Context, Decoder, Encoder};

const MODEL_CTX: usize = 29;
const MODEL_LIMIT: usize = 400;

/// An arithmetic encoder that encodes one bit at a time, with a given
/// probability expressed as a 16-bit integer.
pub struct BitonicEncoder<'a> {
    /// The output bitstream.
    output: &'a mut Vec<u8>,
    /// The low side of the range.
    low: u32,
    /// The high side of the range.
    high: u32,
}

impl<'a> BitonicEncoder<'a> {
    pub fn new(output: &'a mut Vec<u8>) -> Self {
        Self {
            output,
            low: 0,
            high: 0xffffffff,
        }
    }

    /// Encode the bit 'bit' with probability 'prob' in the range 0..65536.
    /// Return the number of bytes written.
    pub fn encode(&mut self, bit: bool, prob: u16) -> usize {
        debug_assert!(self.high > self.low);

        // Figure out the mid point of the range, depending on the probability.
        let gap = (self.high - self.low) as u64;
        let scale = (gap * prob as u64) >> 16;
        let mid = self.low + scale as u32;
        debug_assert!(self.high > mid && mid >= self.low);

        // Select the sub-range based on the bit.
        if bit {
            self.high = mid;
        } else {
            self.low = mid + 1;
        }

        let mut wrote = 0;
        // Write the identical leading bytes.
        while (self.high ^ self.low) < (1 << 24) {
            self.output.push((self.high >> 24) as u8);
            self.high = (self.high << 8) + 0xff;
            self.low <<= 8;
            wrote += 1;
        }
        wrote
    }

    /// Seal the stream by flushing the state.
    pub fn finalize(&mut self) -> usize {
        // Encode a zero-probability token which flushes the state.
        self.encode(true, 0)
    }

    /// Only use this method for testing.
    pub fn encode_array(&mut self, vals: &[bool], prob: &[u16]) {
        assert_eq!(vals.len(), prob.len());
        for i in 0..vals.len() {
            self.encode(vals[i], prob[i]);
        }
        self.finalize();
    }
}

/// An arithmetic decoder that decodes one bit at a time, with a given
/// probability expressed as a 16-bit integer. See 'BitonicEncoder' for details.
pub struct BitonicDecoder<'a> {
    /// The input bit stream (read from the beginning).
    input: &'a [u8],
    /// Marks the location in the bitstream.
    cursor: usize,
    /// The low point of the range.
    low: u32,
    /// The high point of the range.
    high: u32,
    /// The current state.
    state: u32,
}

impl<'a> BitonicDecoder<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        assert!(input.len() >= 4);
        let mut cursor = 0;
        let mut state: u32 = 0;
        for _ in 0..4 {
            state = state << 8 | input[cursor] as u32;
            cursor += 1;
        }

        Self {
            input,
            cursor,
            low: 0,
            high: 0xffffffff,
            state,
        }
    }

    /// Return the number of bytes consumed from the input.
    pub fn read(&self) -> usize {
        self.cursor
    }

    /// Decode one bit with a probability 'prob' in the range 0..65536.
    pub fn decode(&mut self, prob: u16) -> Option<bool> {
        debug_assert!(self.high > self.low);
        debug_assert!(self.high >= self.state && self.low <= self.state);

        // Figure out the mid point of the range, depending on the probability.
        let gap = (self.high - self.low) as u64;
        let scale = (gap * prob as u64) >> 16;
        let mid = self.low + scale as u32;
        debug_assert!(self.high > mid && mid >= self.low);

        // Figure out which bit we extract based on where the state falls in the
        // range.
        let bit = self.state <= mid;

        // Select the sub-range based on the bit.
        if bit {
            self.high = mid;
        } else {
            self.low = mid + 1;
        }

        // Clear the identical leading bytes.
        while (self.high ^ self.low) < (1 << 24) {
            // Not enough bits in the input.
            if self.cursor == self.input.len() {
                return None;
            }
            self.high = (self.high << 8) + 0xff;
            self.low <<= 8;
            self.state = (self.state << 8) + self.input[self.cursor] as u32;
            self.cursor += 1;
        }

        Some(bit)
    }

    /// Only use this method for testing.
    pub fn decode_array(&mut self, prob: &[u16]) -> Option<Vec<bool>> {
        let mut res = Vec::new();
        for &p in prob {
            res.push(self.decode(p)?);
        }
        Some(res)
    }
}

#[test]
fn test_encoder_decoder() {
    let mut stream = Vec::new();
    let mut encoder = BitonicEncoder::new(&mut stream);
    encoder.encode(false, 255);
    encoder.encode(false, 255);
    encoder.encode(true, 255);
    encoder.encode(false, 255);
    encoder.finalize();

    let mut decoder = BitonicDecoder::new(&stream);
    let _ = decoder.decode(255);
    let _ = decoder.decode(255);
    let _ = decoder.decode(255);
    let _ = decoder.decode(255);
}

#[test]
fn test_encoder_decoder_array() {
    // Define a few probabilities (in the range 0..1<<16)
    let p = 60000_u16;
    let q = 1400_u16;
    let r = 25120_u16;

    // A test vector.
    let test_vector = [
        true, false, true, false, true, true, false, true, false, true, false,
        true, false, false, false, true, false, false,
    ];

    {
        let mut stream = Vec::new();

        let mut encoder = BitonicEncoder::new(&mut stream);
        let prob = [p, p, p, p, p, p, p, p, p, p, p, p, p, p, p, p, p, p];

        encoder.encode_array(&test_vector, &prob);

        let mut decoder = BitonicDecoder::new(&stream);
        let res = decoder.decode_array(&prob).unwrap();
        assert_eq!(res, test_vector);
    }

    {
        let mut stream = Vec::new();
        let mut encoder = BitonicEncoder::new(&mut stream);
        let prob = [p, q, q, r, p, p, r, r, p, p, q, q, q, q, r, p, q, p];

        encoder.encode_array(&test_vector, &prob);

        let mut decoder = BitonicDecoder::new(&stream);
        let res = decoder.decode_array(&prob).unwrap();
        assert_eq!(res, test_vector);
    }
}

/// A trait that defines the interface for making predictions.
pub trait Model {
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

impl<const CTX_SIZE_BITS: usize, const LIMIT: usize>
    BitwiseModel<CTX_SIZE_BITS, LIMIT>
{
    fn new() -> Self {
        Self {
            ctx: 0,
            cache: vec![(1, 1); 1 << CTX_SIZE_BITS],
        }
    }
}

impl<const CTX_SIZE_BITS: usize, const LIMIT: usize> Model
    for BitwiseModel<CTX_SIZE_BITS, LIMIT>
{
    /// Return a probability prediction in the 16-bit range using the
    /// 'CTX_SIZE_BITS' LSB bits in 'ctx'.
    fn predict(&self) -> u16 {
        let key = self.ctx % (1 << CTX_SIZE_BITS);
        let (set, cnt) = self.cache[key as usize];
        debug_assert!(cnt < 1024);
        let a = set as u64;
        let b = 1 + cnt as u64;

        // This is equivalent to (a * (1<<16)) / b;
        ((a * (RECIPROCAL_U32[b as usize] as u64)) >> 16) as u16
    }

    /// Update the probability of the context 'ctx', considering the first
    /// 'CTX_SIZE_BITS' LSB bits, with the bit 'bit'.
    fn update(&mut self, bit: u8) {
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

/// Start with context of 4 bits.
const DMC_LEVELS: usize = 4;

pub struct DMCModel {
    state: usize,
    /// Maps the current state to the next (0, 1) states.
    states: Vec<[usize; 2]>,
    /// Records the counts (events seen) for each edge.
    counts: Vec<[f32; 2]>,
}

impl DMCModel {
    pub fn new(levels: usize) -> Self {
        let mut model = DMCModel {
            state: 0,
            states: Vec::new(),
            counts: Vec::new(),
        };
        model.init(levels);
        model
    }

    /// Create the initial state machine that has a tree-structure with 'layers'
    fn init(&mut self, layers: usize) {
        assert_eq!(self.states.len(), 0);
        assert_eq!(self.counts.len(), 0);
        assert_eq!(self.state, 0);
        let _ = self.allocate_new_state([0, 0], [0.1, 0.1]);
        for layer in 1..layers {
            let len = (1 << layer) - 1;
            for _ in 0..len {
                let left = self.allocate_new_state([0, 0], [0.1, 0.1]);
                let right = self.allocate_new_state([0, 0], [0.1, 0.1]);
                self.states[left / 2][0] = left;
                self.states[left / 2][1] = right;
            }
        }
    }

    /// Allocate a new state and return it's index.
    fn allocate_new_state(
        &mut self,
        next: [usize; 2],
        counts: [f32; 2],
    ) -> usize {
        self.states.push(next);
        self.counts.push(counts);
        self.counts.len() - 1
    }

    fn verify(&self) {
        let len = self.counts.len();
        for i in 0..len {
            let t0 = self.counts[i][0];
            let t1 = self.counts[i][1];
            assert!(!t0.is_zero() && !t0.is_nan());
            assert!(!t1.is_zero() && !t1.is_nan());
            assert!(self.states[i][0] < len && self.states[i][1] < len);
        }
    }

    pub fn try_clone(&mut self, edge: usize) {
        let curr = self.state;
        let from = curr;
        let to = self.states[curr][edge];

        // This is the cost of the edge that we want to redirect.
        let edge_count = self.counts[from][edge];
        let sum = self.counts[to][0] + self.counts[to][1];

        // Early exit good edges.
        if edge_count < 2. || sum <= 2. + edge_count {
            return;
        }

        assert!(edge_count != 0.);
        assert!(sum != 0.);
        assert!(edge_count != sum);

        // Create a new node.
        let tc = self.counts[to];
        let r = edge_count / sum;
        assert!(r != 1.0);
        let tc0 = tc[0] * r;
        let tc1 = tc[1] * r;
        self.counts[to][0] -= tc0;
        self.counts[to][1] -= tc1;
        let new = self.allocate_new_state(self.states[to], [tc0, tc1]);
        // Register the new node.
        self.states[curr][edge] = new;
    }

    /// Print a dotty graph of the state machine.
    pub fn dump(&self) {
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

impl Model for DMCModel {
    /// Return a probability prediction in the 16-bit range.
    fn predict(&self) -> u16 {
        self.verify();
        let counts = self.counts[self.state];
        let a = counts[0];
        let b = counts[0] + counts[1];
        assert!(!b.is_nan());
        assert!(!b.is_zero());
        ((a / b) * 65536.) as u16
    }

    /// Update the probability of the model with the bit 'bit'.
    /// Advance to the next state, and update the counts.
    fn update(&mut self, bit: u8) {
        self.try_clone(bit as usize);
        self.counts[self.state][bit as usize] += 1.;
        self.state = self.states[self.state][bit as usize];
        self.verify();
    }
}

#[test]
fn test_dmc_dump() {
    let text = "this is a message. this is"; // a message.  this is a message.";
    let text = text.as_bytes();
    let mut model = DMCModel::new(DMC_LEVELS);

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

/// Adaptive Arithmetic Encoder. The encoder predicts the probability of the
/// next bit and uses an arithmetic encoder to compress the data based on the
/// prediction. The higher the accuracy of the prediction the higher the
/// compression rate.
pub struct AdaptiveArithmeticEncoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
}

/// Adaptive Arithmetic Decoder. See AdaptiveArithmeticEncoder for details.
pub struct AdaptiveArithmeticDecoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
}

impl<'a> Encoder<'a> for AdaptiveArithmeticEncoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>, _ctx: Context) -> Self {
        AdaptiveArithmeticEncoder { input, output }
    }

    fn encode(&mut self) -> usize {
        self.output.extend(ARITH_SIG);
        write32(self.input.len() as u32, self.output);
        let mut wrote = ARITH_SIG.len() + 4;

        let mut encoder = BitonicEncoder::new(self.output);
        let mut model = BitwiseModel::<MODEL_CTX, MODEL_LIMIT>::new();

        // For each byte:
        for b in self.input {
            // For each bit:
            for j in 0..8 {
                let bit = (b >> (7 - j)) & 0x1;
                // Make a prediction, decode a bit, and update the model.
                let p = model.predict();
                wrote += encoder.encode(bit != 0, p);
                model.update(bit);
            }
        }
        wrote += encoder.finalize();
        wrote
    }
}

impl<'a> Decoder<'a> for AdaptiveArithmeticDecoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        AdaptiveArithmeticDecoder { input, output }
    }

    fn decode(&mut self) -> Option<(usize, usize)> {
        let mut cursor = 0;
        // Check the signature.
        if !match_signature(self.input, &ARITH_SIG) {
            return None;
        }
        cursor += ARITH_SIG.len();

        // Read the length part.
        let length = read32(&self.input[cursor..])? as usize;
        cursor += 4;
        let stream = &self.input[cursor..];

        let mut decoder = BitonicDecoder::new(stream);
        let mut model = BitwiseModel::<MODEL_CTX, MODEL_LIMIT>::new();

        let mut wrote = 0;
        // For each byte:
        for _ in 0..length {
            let mut byte: u8 = 0;
            // For each bit:
            for _ in 0..8 {
                // Make a prediction, decode a bit, and update the model.
                let p = model.predict();
                let bit = decoder.decode(p)?;
                model.update(bit as u8);
                // Save the bit.
                byte = (byte << 1) + bit as u8;
            }
            self.output.push(byte);
            wrote += 1;
        }
        Some((decoder.read() + cursor, wrote))
    }
}

#[test]
fn test_encoder_decoder_protocol() {
    let text = "this is a message. this is a message.  this is a message.";
    let text = text.as_bytes();
    let mut comp: Vec<u8> = Vec::new();
    let mut decomp: Vec<u8> = Vec::new();
    let ctx = Context::new(9, 1 << 20);

    let _ = AdaptiveArithmeticEncoder::new(text, &mut comp, ctx).encode();
    let _ = AdaptiveArithmeticDecoder::new(&comp, &mut decomp).decode();
    assert_eq!(text, decomp);
}

#[test]
fn test_encoder_decoder_zeros() {
    let zeros = vec![0; 1 << 8];
    let mut comp: Vec<u8> = Vec::new();
    let mut decomp: Vec<u8> = Vec::new();
    let ctx = Context::new(9, 1 << 20);

    let _ = AdaptiveArithmeticEncoder::new(&zeros, &mut comp, ctx).encode();
    let _ = AdaptiveArithmeticDecoder::new(&comp, &mut decomp).decode();
    assert_eq!(zeros, decomp);
}
