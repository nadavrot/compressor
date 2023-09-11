//! A module that implements a simple tANS entropy encoder.

use crate::bitvector::Bitvector;
use crate::coding::hist::{num_bits, Histogram};
use crate::{Context, Decoder, Encoder};

type DecodeTable = Vec<(u32, u8)>;

/// A class that creates the encode/decode table and is used by the encoder and
/// decoder.
struct Coder<const ALPHABET: usize, const TABLESIZE: usize> {
    /// This is the main encoder table.
    /// A table of [Symbol x State] (use the get_state accessor).
    encode_table: Vec<u16>,
    /// Maps symbol to the max state that can encode this symbol.
    max_state: Vec<u32>,
    /// This is the main decoder table.
    /// Maps each state to (next_state, sym)
    decode_table: DecodeTable,
    /// The normalized histogram
    norm_hist: Vec<u32>,
}

impl<const ALPHABET: usize, const TABLESIZE: usize> Coder<ALPHABET, TABLESIZE> {
    pub fn new() -> Self {
        Self {
            encode_table: vec![0; ALPHABET * TABLESIZE * 2],
            max_state: vec![0; ALPHABET],
            decode_table: vec![(0, 0); TABLESIZE * 2],
            norm_hist: Vec::new(),
        }
    }

    /// Check if 'state' is a valid state.
    fn check_state(state: usize) {
        debug_assert!(state >= TABLESIZE && state <= TABLESIZE * 2);
    }

    /// Initialize the coder with the input data and create the
    /// encoder/decoder tables.
    pub fn init_from_input(&mut self, input: &[u8]) {
        let mut hist = Histogram::<ALPHABET>::from_data(input);
        hist.normalize(TABLESIZE);
        let norm_hist = hist.get_bins();
        self.init_from_histogram(norm_hist)
    }

    /// Create the encode/decode tables from a valid normalized histogram.
    pub fn init_from_histogram(&mut self, norm_hist: &[u32]) {
        assert!(Self::is_valid_histogram(norm_hist));
        assert!(self.norm_hist.is_empty(), "Can't init the coder twice");
        self.norm_hist.extend(norm_hist.iter());
        let state_list = self.spread_symbols(norm_hist);
        self.create_tables(norm_hist, &state_list);
    }

    /// Return True if the normalized histogram is valid.
    pub fn is_valid_histogram(norm_hist: &[u32]) -> bool {
        let mut sum: u64 = 0;
        // Sum the elements without overflowing the u32 accumulator.
        for val in norm_hist {
            sum += *val as u64;
        }
        norm_hist.len() == ALPHABET && sum == TABLESIZE as u64
    }

    /// Spread the symbols using Yann's method, which is to randomly place the
    /// symbols around the array, minimizing the distance between symbols
    /// (not grouping them together).
    /// http://fastcompression.blogspot.com/2014/02/fse-distributing-symbol-values.html
    fn spread_symbols(&self, sym_occurrences: &[u32]) -> Vec<u8> {
        let mut state_table: Vec<u8> = vec![0; TABLESIZE];
        // This is a large prime number. We skip around the array with a large
        // prime number to hit every element once.
        let step = 118081;
        let mut pos: usize = 0;

        for (sym, occ) in sym_occurrences.iter().enumerate() {
            for _ in 0..*occ {
                state_table[pos % TABLESIZE] = sym as u8;
                pos += step;
            }
        }
        // The lowest common denominator of the table at the prime is 1,
        // so we know that the cycle size will be the size of the table.
        debug_assert!(pos % TABLESIZE == 0);
        state_table
    }

    /// Return a reference to the encoding table.
    pub fn get_enc_state(&mut self, sym: usize, state: usize) -> &mut u16 {
        debug_assert!(sym < ALPHABET && state < TABLESIZE * 2);
        &mut self.encode_table[(sym * TABLESIZE * 2) + state]
    }

    pub fn get_max_state(&self, sym: usize) -> usize {
        self.max_state[sym] as usize
    }

    /// Given given 'state', a state in the decode table, the method returns a
    /// pair of (new_state, sym) for the decoded symbol and the next state.
    pub fn get_dec_state(&self, state: usize) -> (u32, u8) {
        self.decode_table[state]
    }

    /// Creates the encode/decode tables. 'normalized_occurrences' is the
    /// normalized histogram that accumulates to the sum of the table.
    /// 'state_list' maps a symbol to each state (see spread_symbols).
    /// http://www.ezcodesample.com/abs/abs_article.html
    /// and
    /// http://cbloomrants.blogspot.com/2014/02/02-04-14-understanding-ans-6.html
    /// Reference: cbloom "make_tables1".
    fn create_tables(&mut self, norm_hist: &[u32], state_list: &[u8]) {
        debug_assert!(Self::is_valid_histogram(norm_hist));
        assert!(state_list.len() == TABLESIZE, "Invalid table size");

        // Place the symbols in the table at an offset based on their frequency,
        // such that each symbol is placed between F and 2F.
        // Unlike the table that's shown here:
        // http://www.ezcodesample.com/abs/abs_article.html
        for sym in 0..ALPHABET {
            let is_zero = norm_hist[sym] == 0;
            self.max_state[sym] = if is_zero { 0 } else { norm_hist[sym] - 1 };
        }

        // For each state in the table:
        for to_state in 0..TABLESIZE {
            // Map the state to the symbol:
            let sym = state_list[to_state];

            // Keep track the highest state for each symbol.
            let from_state = self.max_state[sym as usize];
            self.max_state[sym as usize] += 1;

            // Fill the encode table.
            let entry = self.get_enc_state(sym as usize, from_state as usize);
            *entry = (to_state + TABLESIZE) as u16;

            // Fill the decode table.
            debug_assert_eq!(self.decode_table[to_state + TABLESIZE].0, 0);
            self.decode_table[to_state + TABLESIZE] = (from_state, sym);
        }

        if cfg!(debug_assertions) {
            self.verify(norm_hist);
        }
    }

    /// Verify the encode and decode tables after they were created.
    pub fn verify(&mut self, norm_hist: &[u32]) {
        // Numbers in every row are larger than the number of that row.
        for row in 1..TABLESIZE * 2 {
            for sym in 0..ALPHABET {
                let entry = self.get_enc_state(sym, row);
                debug_assert!(*entry == 0 || row < (*entry as usize));
            }
        }
        // Numbers in all columns are sorted.
        for sym in 0..ALPHABET {
            let mut prev = 0;
            for row in 1..TABLESIZE {
                let entry = self.get_enc_state(sym, row);
                debug_assert!(prev == 0 || *entry == 0 || *entry > prev);
                prev = *entry;
            }
        }

        // Check that the symbols are placed in the range F..2F, where F is
        // the normalized frequency.
        for sym in 0..ALPHABET {
            if norm_hist[sym] == 0 {
                continue;
            }
            // Reference make_tables1 by cbloom
            // https://www.cbloom.com/src/ans_learning.cpp
            let max_state = self.max_state[sym];
            let f = norm_hist[sym];
            // The states for the symbols are spread between F and 2F.
            debug_assert!(max_state == f * 2 - 1);
            // Check that the step that brings the state down works.
            let above = max_state;
            let next_iter = above / 2;
            let entry = self.get_enc_state(sym, next_iter as usize);
            Self::check_state(*entry as usize);
        }
    }
}

impl<const ALPHABET: usize, const TABLESIZE: usize> Coder<ALPHABET, TABLESIZE> {
    /// Serialize the normalized histogram as a list of variable-length encoded
    /// integers. Each number is encoded as a sequence of numbers until a number
    /// below 255 is found (just like the lz4 encoding).
    /// Return the number of bytes saved.
    fn serialize(&mut self, output: &mut Vec<u8>) -> usize {
        use crate::utils::variable_length_encoding::encode;
        let mut written = 0;
        for elem in &self.norm_hist {
            written += encode(*elem, output);
        }
        written
    }

    /// Load the serialized normalized histogram. This uses the lz4 variable
    /// length encoding. Check the encoder for more details.
    fn deserialize(input: &[u8]) -> Option<(Vec<u32>, usize)> {
        use crate::utils::variable_length_encoding::decode;

        let mut cursor = 0;
        let mut result: Vec<u32> = Vec::new();

        // For each symbol:
        for _ in 0..ALPHABET {
            let (read, val) = decode(&input[cursor..])?;
            cursor += read;
            result.push(val);
        }
        Some((result, cursor))
    }
}

/// A simple tANS encoder.
pub struct SimpleEncoder<'a, const ALPHABET: usize, const TABLESIZE: usize> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
    /// The coder that manages the encode/decode tables.
    coder: Coder<ALPHABET, TABLESIZE>,
}

impl<'a, const ALPHABET: usize, const TABLESIZE: usize>
    SimpleEncoder<'a, ALPHABET, TABLESIZE>
{
    /// Encode the input into the bitvector using the calculated metadata.
    fn encode_data(&mut self, input: &[u8], bv: &mut Bitvector) {
        let mut state: u32 = 2 * TABLESIZE as u32 - 1;
        for sym in input.iter().rev() {
            self.encode_one_symbol(&mut state, *sym, bv);
        }
        let val = state as usize - TABLESIZE;
        let table_log = num_bits(TABLESIZE as u32 - 1) as usize;
        bv.push_word(val as u64, table_log);
    }

    /// Encode the input buffer and return the output.
    fn encode_impl(&mut self) -> usize {
        // Initialize the coder.
        self.coder.init_from_input(self.input);

        let mut bv = Bitvector::new();
        // Encode the data.
        self.encode_data(self.input, &mut bv);

        // Serialize the coder and the bitstream.
        let mut wrote = self.coder.serialize(self.output);
        wrote += bv.serialize(self.output);
        wrote
    }

    // Encode a single symbol (character).
    fn encode_one_symbol(
        &mut self,
        state: &mut u32,
        sym: u8,
        bv: &mut Bitvector,
    ) {
        Coder::<ALPHABET, TABLESIZE>::check_state(*state as usize);
        debug_assert!(ALPHABET > sym as usize, "Invalid symbol");

        let max_state = self.coder.get_max_state(sym as usize) as u32;

        // Re-normalize: bring the state back to the encodable range.
        while *state >= max_state {
            let lowest_bit = *state & 0x1;
            *state /= 2;
            bv.push_word(lowest_bit as u64, 1);
        }

        *state =
            *self.coder.get_enc_state(sym as usize, *state as usize) as u32;
        Coder::<ALPHABET, TABLESIZE>::check_state(*state as usize);
    }
}

/// A simple tANS decoder.
pub struct SimpleDecoder<'a, const ALPHABET: usize, const TABLESIZE: usize> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
    /// The coder that manages the encode/decode tables.
    coder: Coder<ALPHABET, TABLESIZE>,
}

/// Serialization methods.
impl<'a, const ALPHABET: usize, const TABLESIZE: usize>
    SimpleDecoder<'a, ALPHABET, TABLESIZE>
{
    /// Try to decode the input, and return the number of bytes read and written
    /// if the encoding was a valid encoding.
    fn decode_impl(&mut self) -> Option<(usize, usize)> {
        if self.input.is_empty() {
            return None;
        }

        // Deserialize the normalized histogram.
        let (hist, read) =
            Coder::<ALPHABET, TABLESIZE>::deserialize(self.input)?;
        if !Coder::<ALPHABET, TABLESIZE>::is_valid_histogram(&hist) {
            return None;
        }
        self.coder.init_from_histogram(&hist);

        let (mut bv, read1) = Bitvector::deserialize(&self.input[read..])?;
        let written = self.decode_data(&mut bv)?;
        Some((read + read1, written))
    }
}

/// Compression logic methods.
impl<'a, const ALPHABET: usize, const TABLESIZE: usize>
    SimpleDecoder<'a, ALPHABET, TABLESIZE>
{
    // Extract a single character from the bitstream.
    #[must_use]
    fn decode_one_symbol(
        &self,
        bv: &mut Bitvector,
        state: &mut u32,
    ) -> Option<u8> {
        Coder::<ALPHABET, TABLESIZE>::check_state(*state as usize);

        let (new_state, sym) = self.coder.get_dec_state(*state as usize);

        // Make sure that there are no loops in the decoder.
        debug_assert!(new_state < *state);
        *state = new_state;

        // Re-normalize the state and bring it to the valid range.
        while TABLESIZE > *state as usize && !bv.is_empty() {
            let bit = bv.pop_word(1);
            *state <<= 1;
            *state |= bit as u32;
        }

        Some(sym)
    }

    /// Read a string from the bitvector.
    #[must_use]
    fn decode_data(&mut self, bv: &mut Bitvector) -> Option<usize> {
        let table_log = num_bits(TABLESIZE as u32 - 1) as usize;
        if bv.len() < table_log {
            return None;
        }
        let mut written = 0;
        let mut state: u32 = TABLESIZE as u32 + bv.pop_word(table_log) as u32;
        while !bv.is_empty() {
            let sym = self.decode_one_symbol(bv, &mut state)?;
            self.output.push(sym);
            written += 1;
        }
        Some(written)
    }
}

impl<'a, const ALPHABET: usize, const TABLESIZE: usize> Encoder<'a>
    for SimpleEncoder<'a, ALPHABET, TABLESIZE>
{
    fn new(input: &'a [u8], output: &'a mut Vec<u8>, _ctx: Context) -> Self {
        SimpleEncoder {
            input,
            output,
            coder: Coder::new(),
        }
    }

    fn encode(&mut self) -> usize {
        self.encode_impl()
    }
}

impl<'a, const ALPHABET: usize, const TABLESIZE: usize> Decoder<'a>
    for SimpleDecoder<'a, ALPHABET, TABLESIZE>
{
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        SimpleDecoder {
            input,
            output,
            coder: Coder::new(),
        }
    }

    fn decode(&mut self) -> Option<(usize, usize)> {
        self.decode_impl()
    }
}
