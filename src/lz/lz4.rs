//! An LZ4 block implementation, based on the documentation in
//! <https://github.com/lz4/lz4/blob/dev/doc/lz4_Block_format.md>

use std::ops::Range;

use crate::lz::matcher::Matcher;
use crate::{Decoder, Encoder};

/// An LZ4 Encoder.
pub struct LZ4Encoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
}

impl<'a> LZ4Encoder<'a> {
    /// Encode a single packet into the lz4 stream 'output'. The fields are
    /// The 'literals', and the match operator that is stored in an *unbiased*
    /// 'offset' and a valid match length. If 'is_last' is set then the last
    /// offset block is not encoded. Returns the number of bytes written.
    fn encode_lz4_packet(
        &mut self,
        literals: &[u8],
        offset: u16,
        match_length: usize,
        is_last: bool,
    ) -> usize {
        let mut written = 0;
        let match_len = if is_last {
            0
        } else {
            assert!(match_length >= 4, "Offset must be greater than 4");
            match_length - 4
        };

        // Push the token (the two 4-bit parts).
        let lit_len = literals.len();
        let high = if lit_len > 15 { 15 } else { lit_len as u8 };
        let low = if match_len > 15 { 15 } else { match_len as u8 };
        self.output.push((high << 4) | low);
        written += 1;

        // A procedure for encoding values over 15 in additional bytes.
        let encode = |&x, output: &mut Vec<u8>| -> usize {
            let mut written = 0;
            let mut x = x;
            if x >= 15 {
                x -= 15;
                loop {
                    if x < 255 {
                        output.push(x as u8);
                        written += 1;
                        break;
                    }
                    output.push(255);
                    written += 1;
                    x -= 255;
                }
            }
            written
        };

        written += encode(&lit_len, self.output);

        // Push literals.
        self.output.extend(literals.iter());
        written += literals.len();

        if is_last {
            return written;
        }

        // Push little endian offset field.
        self.output.push(offset as u8);
        self.output.push((offset >> 8) as u8);
        written += 2;
        written += encode(&match_len, self.output);
        written
    }

    fn encode_impl(&mut self) -> usize {
        let mut written = 0;
        let len = self.input.len();

        // Encoding rules:
        // https://github.com/lz4/lz4/blob/dev/doc/lz4_Block_format.md#end-of-block-conditions

        // Rule #3: blocks < 13 bytes cannot be compressed.
        if len < 13 {
            return self.encode_lz4_packet(self.input, 0, 0, true);
        }

        // Construct a matcher.
        // Rule 2:The last 5 bytes are always literals. Don't try to match them.
        let matcher = Matcher::<65536, 65536>::new(&self.input[..(len - 5)]);

        let mut last_encoded = 0;
        for (lit, mat) in matcher {
            let literals = &self.input[lit.clone()];

            // The last match must start at least 12 bytes before the block end.
            if lit.end + 12 >= len {
                let literals = &self.input[lit.start..len];
                written += self.encode_lz4_packet(literals, 0, 0, true);
                return written;
            }

            written += self.encode_lz4_packet(
                literals,
                (lit.end - mat.start) as u16,
                mat.len(),
                false,
            );
            last_encoded = lit.end + mat.len();
        }

        // Encode the last literal block.
        let last_lit = &self.input[last_encoded..];
        written += self.encode_lz4_packet(last_lit, 0, 0, true);
        written
    }
}

/// An LZ4 Decoder.
pub struct LZ4Decoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
    /// Points to the current byte to process.
    cursor: usize,
}

impl<'a> LZ4Decoder<'a> {
    pub fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        Self {
            input,
            output,
            cursor: 0,
        }
    }

    fn decode_following_bytes(&mut self, x: usize) -> Option<usize> {
        let mut x: usize = x;
        let len = self.input.len();
        if x == 15 {
            loop {
                if self.cursor >= len {
                    return None;
                }
                let next = self.input[self.cursor] as usize;
                x += next;
                self.cursor += 1;
                if next != 255 {
                    return Some(x);
                }
            }
        }
        Some(x)
    }

    /// Decode a single packet from the lz4 stream 'input'.
    /// Returns the unbiased match operator Region and a reference to the
    /// literals. The method updates the cursor.
    fn decode_lz4_packet(
        &mut self,
        end_of_buffer: usize,
    ) -> Option<(Range<usize>, &'a [u8])> {
        let num_literals = (self.input[self.cursor] >> 4) as usize;
        let match_len = (self.input[self.cursor] & 0xf) as usize;
        self.cursor += 1;

        let num_literals = self.decode_following_bytes(num_literals)?;

        let end = self.cursor + num_literals;
        if end > self.input.len() {
            return None;
        }
        let literal_ref = &self.input[self.cursor..end];
        self.cursor += num_literals;

        // Handle the half-token end of stream.
        if self.cursor == end_of_buffer {
            return Some((0..0, literal_ref));
        }

        if self.cursor + 1 >= self.input.len() {
            return None;
        }

        let offset_low = self.input[self.cursor];
        let offset_high = self.input[self.cursor + 1];
        self.cursor += 2;
        let match_len = self.decode_following_bytes(match_len)?;

        let offset = (offset_low as u16) + ((offset_high as u16) << 8);
        let reg = offset as usize..offset as usize + match_len + 4;
        Some((reg, literal_ref))
    }

    /// Decode the input parameter. Returns the number of bytes consumed and the
    /// number of bytes written if the operation succeeded.
    fn decode_impl(&mut self) -> Option<(usize, usize)> {
        assert_eq!(self.output.len(), 0);
        self.cursor = 0;
        let mut written = 0;

        let len = self.input.len();
        if len == 0 {
            return Some((0, 0));
        }
        while self.cursor < len {
            let (match_op, literals) = self.decode_lz4_packet(len)?;
            self.output.extend(literals.iter());
            written += literals.len();
            if match_op.start == 0 {
                return Some((self.cursor, written));
            }
            let len = self.output.len();

            // Check that the match window does not overflow
            if match_op.start > len {
                return None;
            }
            // Copy the match into the output stream.
            for i in 0..match_op.len() {
                self.output.push(self.output[len - match_op.start + i]);
            }
            written += match_op.len();
        }
        None
    }
}

impl<'a> Encoder<'a> for LZ4Encoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        Self { input, output }
    }

    fn encode(&mut self) -> usize {
        self.encode_impl()
    }
}

impl<'a> Decoder<'a> for LZ4Decoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        Self {
            input,
            output,
            cursor: 0,
        }
    }

    fn decode(&mut self) -> Option<(usize, usize)> {
        self.decode_impl()
    }
}
