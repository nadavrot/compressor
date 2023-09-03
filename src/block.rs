//! This module handles the encoding and decoding of each block in the file.
//! In this module we decide the order of transformations, such as matching
//! and entropy encoding.

use crate::coding::simple::{SimpleDecoder, SimpleEncoder};
use crate::lz::matcher::select_matcher;
use crate::nop::{NopDecoder, NopEncoder};
use crate::utils::signatures::{match_signature, BLOCK_SIG};

use crate::utils::array_encoding::decode as decode_arr;
use crate::utils::array_encoding::encode as encode_arr;

use crate::utils::variable_length_encoding::decode_array32 as decode_vl32;
use crate::utils::variable_length_encoding::encode_array32 as encode_vl32;

use crate::{Context, Decoder, Encoder};

type EncoderTy<'a> = SimpleEncoder<'a, 256, 4096>;
type DecoderTy<'a> = SimpleDecoder<'a, 256, 4096>;

//. Try to perform entropy encoding, but if it fails use nop encoding.
fn encode_entropy(input: &[u8], ctx: Context) -> Vec<u8> {
    let mut encoded: Vec<u8> = Vec::new();
    let new_size = EncoderTy::new(input, &mut encoded, ctx).encode();

    if new_size < input.len() {
        return encoded;
    }
    encoded.clear();
    let _ = NopEncoder::new(input, &mut encoded, ctx).encode();
    encoded
}

/// Try to perform entropy encoding, but if it fails use nop encoding.
fn decode_entropy(input: &[u8]) -> Option<Vec<u8>> {
    let mut decoded: Vec<u8> = Vec::new();

    if DecoderTy::new(input, &mut decoded).decode().is_some() {
        return Some(decoded);
    }

    assert_eq!(decoded.len(), 0);
    if NopDecoder::new(input, &mut decoded).decode().is_some() {
        return Some(decoded);
    }

    None
}

/// Drives the encoding of a single block.
pub struct BlockEncoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
    /// Encoder context.
    ctx: Context,
}

impl<'a> BlockEncoder<'a> {
    fn encode_buffer(input: &'a [u8], ctx: Context) -> Vec<u8> {
        let matcher = select_matcher(ctx.level, input);

        let mut lits: Vec<u8> = Vec::new();
        let mut lit_lens: Vec<u32> = Vec::new();
        let mut mat_offs_high: Vec<u8> = Vec::new();
        let mut mat_offs_low: Vec<u8> = Vec::new();
        let mut mat_lens: Vec<u32> = Vec::new();

        let mut prev_match = 0;
        for (lit, mat) in matcher {
            // Serialize the literals and the length of each segment.
            let literals = &input[lit.clone()];
            lits.extend(literals);
            lit_lens.push(lit.len() as u32);

            // Serialize the matches. Calculate the normalized offset, and
            // encode the length and offset.
            let mut match_offset = lit.end - mat.start;
            // Split the offsets to a high and low parts.

            // Encode consecutive offsets as zeros.
            if prev_match == match_offset {
                match_offset = 0;
            } else {
                prev_match = match_offset;
            }

            mat_offs_high.push((match_offset >> 8) as u8);
            mat_offs_low.push(match_offset as u8);
            mat_lens.push(mat.len() as u32);
        }

        // Turn everything to U8 arrays.
        let mut lit_len_u8: Vec<u8> = Vec::new();
        let mut mat_len_u8: Vec<u8> = Vec::new();

        encode_vl32(&lit_lens, &mut lit_len_u8);
        encode_vl32(&mat_lens, &mut mat_len_u8);

        // Entropy encode what is possible.
        let lit_stream2 = encode_entropy(&lits, ctx);
        let lit_len_stream2 = encode_entropy(&lit_len_u8, ctx);
        let mat_off_high2 = encode_entropy(&mat_offs_high, ctx);
        let mat_off_low2 = encode_entropy(&mat_offs_low, ctx);
        let mat_len_stream2 = encode_entropy(&mat_len_u8, ctx);

        // To the wire!
        let mut result = Vec::new();
        encode_arr(&lit_stream2, &mut result);
        encode_arr(&lit_len_stream2, &mut result);
        encode_arr(&mat_off_high2, &mut result);
        encode_arr(&mat_off_low2, &mut result);
        encode_arr(&mat_len_stream2, &mut result);
        result
    }

    fn encode_impl(&mut self) -> usize {
        // Write the magic signature.
        self.output.extend(BLOCK_SIG);

        // Compress the content and write it to the output.
        let res = Self::encode_buffer(self.input, self.ctx);
        self.output.extend(&res);

        // Bytes written plus the signature.
        res.len() + BLOCK_SIG.len()
    }
}

/// Drives the decoding of a single block.
pub struct BlockDecoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
}

impl<'a> BlockDecoder<'a> {
    fn decode_buffer(input: &'a [u8]) -> Option<(usize, Vec<u8>)> {
        let mut literals: Vec<u8> = Vec::new();
        let mut lit_lens: Vec<u8> = Vec::new();
        let mut mat_offs_high: Vec<u8> = Vec::new();
        let mut mat_offs_low: Vec<u8> = Vec::new();
        let mut mat_lens: Vec<u8> = Vec::new();

        let mut read = 0;
        read += decode_arr(&input[read..], &mut literals)?;
        read += decode_arr(&input[read..], &mut lit_lens)?;
        read += decode_arr(&input[read..], &mut mat_offs_high)?;
        read += decode_arr(&input[read..], &mut mat_offs_low)?;
        read += decode_arr(&input[read..], &mut mat_lens)?;

        let literals2 = decode_entropy(&literals)?;
        let lit_lens2 = decode_entropy(&lit_lens)?;
        let mat_offs_high2 = decode_entropy(&mat_offs_high)?;
        let mat_offs_low2 = decode_entropy(&mat_offs_low)?;
        let mat_lens2 = decode_entropy(&mat_lens)?;

        // Back from U8 to U16 and U32.
        let mut lit_lens3: Vec<u32> = Vec::new();
        let mut mat_offs3: Vec<u16> = Vec::new();
        let mut mat_lens3: Vec<u32> = Vec::new();

        let high_low = mat_offs_high2.iter().zip(mat_offs_low2.iter());

        // Decode the offsets. Zero means that we need to use the previous
        // offset.
        let mut prev_offset = 0;
        for pair in high_low {
            let mut offset = ((*pair.0 as u16) << 8) + (*pair.1 as u16);
            if offset == 0 {
                offset = prev_offset;
            } else {
                prev_offset = offset;
            }
            mat_offs3.push(offset);
        }

        let _ = decode_vl32(&lit_lens2, &mut lit_lens3)?;
        let _ = decode_vl32(&mat_lens2, &mut mat_lens3)?;
        let mut result: Vec<u8> = Vec::new();

        let mut lit_cursor = 0;
        let mut out_cursor = 0;
        for i in 0..lit_lens3.len() {
            let lit_len = lit_lens3[i] as usize;
            let mat_len = mat_lens3[i] as usize;
            let mat_off = mat_offs3[i] as usize;

            // Copy the literals.
            let lit = &literals2[lit_cursor..lit_cursor + lit_len];
            lit_cursor += lit_len;
            out_cursor += lit_len;
            result.extend(lit);

            // Copy the match.
            for i in 0..mat_len {
                result.push(result[out_cursor - mat_off + i]);
            }
            out_cursor += mat_len;
        }

        Some((read, result))
    }

    fn decode_impl(&mut self) -> Option<(usize, usize)> {
        let sig_len = BLOCK_SIG.len();
        if !match_signature(self.input, &BLOCK_SIG) {
            return None;
        }

        // Decode the content.
        let (read, buff) = Self::decode_buffer(&self.input[sig_len..])?;

        self.output.extend(&buff);
        Some((sig_len + read, buff.len()))
    }
}

impl<'a> Encoder<'a> for BlockEncoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>, ctx: Context) -> Self {
        BlockEncoder { input, output, ctx }
    }

    fn encode(&mut self) -> usize {
        self.encode_impl()
    }
}

impl<'a> Decoder<'a> for BlockDecoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        BlockDecoder { input, output }
    }

    fn decode(&mut self) -> Option<(usize, usize)> {
        self.decode_impl()
    }
}
