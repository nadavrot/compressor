//! This module handles the encoding and decoding of each block in the file.
//! In this module we decide the order of transformations, such as matching
//! and entropy encoding.

use crate::bitvector::Bitvector;
use crate::coding::simple::{SimpleDecoder, SimpleEncoder};
use crate::lz::matcher::select_matcher;
use crate::nop::{NopDecoder, NopEncoder};
use crate::pager::{
    DecodeHandlerTy, EncodeHandlerTy, PagerDecoder, PagerEncoder,
};
use crate::utils::signatures::{match_signature, BLOCK_SIG};

use crate::utils::array_encoding::decode as decode_arr;
use crate::utils::array_encoding::encode as encode_arr;

use crate::utils::two_stream_encoding;
use crate::utils::variable_length_encoding::decode_array32 as decode_vl32;
use crate::utils::variable_length_encoding::encode_array32 as encode_vl32;

use crate::{Context, Decoder, Encoder};

/// This is the maximum number of length bits that we allow for offsets. (1<<X)
const MAX_OFFSET_BITS: usize = 24;

/// Selects the size of each entropy unit.
const ENTROPY_PAGE_SIZE: usize = 1 << 18;

/// Encode a list of offsets, with a histogram that favors short indices, into
/// two streams: tokens and extra bits. The tokens are compressed with fse, and
/// the extra bits are encoded into a bitstream. See 'two_stream_encoding' for
/// details.
pub fn encode_offset_stream(input: &[u32], ctx: Context) -> Vec<u8> {
    let mut bv = Bitvector::new();
    let mut tokens = Vec::new();
    let mut encoded = Vec::new();

    // Split the offsets into two streams: tokens and bitvector.
    for val in input {
        tokens.push(two_stream_encoding::encode32(*val, &mut bv) as u8);
    }

    let res = encode_paged_ent(&tokens, ctx, encode_offset_entropy);
    encoded.extend(res);

    // Append the bitstream after the tokens.
    let _ = bv.serialize(&mut encoded);
    encoded
}

/// Decode the list of offsets that were encoded with 'encode_offset_stream'.
pub fn decode_offset_stream(input: &[u8]) -> Option<Vec<u32>> {
    let (read, tokens) = decode_paged_ent(input, decode_offset_entropy)?;

    let (mut bv, bv_read) = Bitvector::deserialize(&input[read..])?;
    // Check that all of the data was read.
    if read + bv_read != input.len() {
        return None;
    }

    let mut res: Vec<u32> = Vec::new();

    // We need to process the values in reverse, because the bits are
    // stored in the bitvector in reverse.
    for tok in tokens.iter().rev() {
        res.push(two_stream_encoding::decode32(*tok as u32, &mut bv));
    }
    res.reverse();
    Some(res)
}

// Perform entropy encoding on an input with valid tokens.
fn encode_offset_entropy(input: &[u8], ctx: Context) -> Vec<u8> {
    let mut encoded: Vec<u8> = Vec::new();
    type EncoderTy<'a> = SimpleEncoder<'a, MAX_OFFSET_BITS, 4096>;
    let _ = EncoderTy::new(input, &mut encoded, ctx).encode();
    encoded
}

// Decode the entropy encoding of a list of valid tokens.
fn decode_offset_entropy(input: &[u8]) -> Option<(usize, Vec<u8>)> {
    let mut decoded: Vec<u8> = Vec::new();
    type DecoderTy<'a> = SimpleDecoder<'a, MAX_OFFSET_BITS, 4096>;
    let (read, _) = DecoderTy::new(input, &mut decoded).decode()?;
    Some((read, decoded))
}

//. Try to perform entropy encoding, but if it fails use nop encoding.
fn ent_or_nop(input: &[u8], ctx: Context) -> Vec<u8> {
    let mut encoded: Vec<u8> = Vec::new();
    type EncoderTy<'a> = SimpleEncoder<'a, 256, 4096>;
    let new_size = EncoderTy::new(input, &mut encoded, ctx).encode();

    if new_size < input.len() {
        return encoded;
    }
    encoded.clear();
    let _ = NopEncoder::new(input, &mut encoded, ctx).encode();
    encoded
}

/// Try to perform entropy encoding, but if it fails use nop encoding.
fn decode_ent_or_nop(input: &[u8]) -> Option<(usize, Vec<u8>)> {
    let mut decoded: Vec<u8> = Vec::new();

    type DecoderTy<'a> = SimpleDecoder<'a, 256, 4096>;
    if let Some((read, _)) = DecoderTy::new(input, &mut decoded).decode() {
        return Some((read, decoded));
    }

    assert_eq!(decoded.len(), 0);
    if let Some((read, _)) = NopDecoder::new(input, &mut decoded).decode() {
        return Some((read, decoded));
    }

    None
}

fn encode_paged_ent(
    input: &[u8],
    ctx: Context,
    callback: EncodeHandlerTy,
) -> Vec<u8> {
    let mut encoded: Vec<u8> = Vec::new();
    let mut encoder = PagerEncoder::new(input, &mut encoded, ctx);
    encoder.set_callback(callback);
    encoder.set_page_size(ENTROPY_PAGE_SIZE);
    let _ = encoder.encode();
    encoded
}

fn decode_paged_ent(
    input: &[u8],
    callback: DecodeHandlerTy,
) -> Option<(usize, Vec<u8>)> {
    let mut decoded: Vec<u8> = Vec::new();
    let mut encoder = PagerDecoder::new(input, &mut decoded);
    encoder.set_callback(callback);
    let (read, _) = encoder.decode()?;
    Some((read, decoded))
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
        // The max offset is 1 << MAX_OFFSET_BITS - 3 to allow the special
        // encoding of offsets.
        let matcher = select_matcher::<16777210, 65536>(ctx.level, input);

        let mut lits: Vec<u8> = Vec::new();
        let mut lit_lens: Vec<u32> = Vec::new();
        let mut mat_offsets: Vec<u32> = Vec::new();
        let mut mat_lens: Vec<u32> = Vec::new();

        let mut prev_off1 = 0;
        let mut prev_off2 = 0;
        let mut prev_off3 = 0;

        for (lit, mat) in matcher {
            // Serialize the literals and the length of each segment.
            let literals = &input[lit.clone()];
            lits.extend(literals);
            lit_lens.push(lit.len() as u32);

            // Calculate the offset to the match.
            let mut match_offset = lit.end - mat.start;

            // Don't encode empty matches. These show up at stream ends.
            if mat.is_empty() {
                match_offset = 0;
            }
            // Add a bias of 3 to allow us to encode previous matches.
            match_offset += 3;

            // Check if we are encoding one of the previous matches.
            if prev_off1 == match_offset {
                match_offset = 0;
            } else if prev_off2 == match_offset {
                match_offset = 1;
            } else if prev_off3 == match_offset {
                match_offset = 2;
            }

            prev_off3 = prev_off2;
            prev_off2 = prev_off1;
            prev_off1 = match_offset;

            // Store the match length and offsets.
            mat_offsets.push(match_offset as u32);
            mat_lens.push(mat.len() as u32);
        }

        // Turn everything to U8 arrays.
        let mut lit_len_u8: Vec<u8> = Vec::new();
        let mut mat_len_u8: Vec<u8> = Vec::new();

        encode_vl32(&lit_lens, &mut lit_len_u8);
        encode_vl32(&mat_lens, &mut mat_len_u8);

        // Entropy encode what is possible.
        let lit_stream2 = encode_paged_ent(&lits, ctx, ent_or_nop);
        let lit_len_stream2 = encode_paged_ent(&lit_len_u8, ctx, ent_or_nop);
        let mat_off_u8 = encode_offset_stream(&mat_offsets, ctx);
        let mat_len_stream2 = encode_paged_ent(&mat_len_u8, ctx, ent_or_nop);

        // To the wire!
        let mut result = Vec::new();
        encode_arr(&lit_stream2, &mut result);
        encode_arr(&lit_len_stream2, &mut result);
        encode_arr(&mat_off_u8, &mut result);
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
        let mut mat_offs: Vec<u8> = Vec::new();
        let mut mat_lens: Vec<u8> = Vec::new();

        let mut read = 0;
        read += decode_arr(&input[read..], &mut literals)?;
        read += decode_arr(&input[read..], &mut lit_lens)?;
        read += decode_arr(&input[read..], &mut mat_offs)?;
        read += decode_arr(&input[read..], &mut mat_lens)?;

        let literals2 = decode_paged_ent(&literals, decode_ent_or_nop)?.1;
        let lit_lens2 = decode_paged_ent(&lit_lens, decode_ent_or_nop)?.1;
        let mat_offs2 = decode_offset_stream(&mat_offs)?;
        let mat_lens2 = decode_paged_ent(&mat_lens, decode_ent_or_nop)?.1;

        let mut lit_lens3: Vec<u32> = Vec::new();
        let mut mat_offs3: Vec<u32> = Vec::new();
        let mut mat_lens3: Vec<u32> = Vec::new();

        // Decode the offsets. Zero means that we need to use the previous
        // offset.
        let mut prev_off1 = 0;
        let mut prev_off2 = 0;
        let mut prev_off3 = 0;

        // Decode the offset (the first 3 values refer to previous offsets).
        for offset in mat_offs2 {
            let off = match offset {
                0 => prev_off1,
                1 => prev_off2,
                2 => prev_off3,
                _ => offset,
            };
            prev_off3 = prev_off2;
            prev_off2 = prev_off1;
            prev_off1 = offset;
            mat_offs3.push(off - 3);
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
