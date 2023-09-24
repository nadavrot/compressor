//! Handles the encoding of the whole file. This module mainly splits the input
//! into chunks and calls the block compressor.

use crate::block::{BlockDecoder, BlockEncoder};
use crate::coding::bitonic::AdaptiveArithmeticDecoder as AAD;
use crate::coding::bitonic::AdaptiveArithmeticEncoder as AAE;
use crate::nop::{NopDecoder, NopEncoder};
use crate::pager::{PagerDecoder, PagerEncoder};
use crate::utils::signatures::{match_signature, ARITH_SIG, FULL_SIG};
use crate::{Context, Decoder, Encoder};

pub struct FullEncoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
    /// Encoder context,
    ctx: Context,
}

/// Try to perform block encoding, but if it's not useful use nop encoding instead.
fn encode_or_nop(input: &[u8], ctx: Context) -> Vec<u8> {
    let mut encoded: Vec<u8> = Vec::new();
    let new_size = BlockEncoder::new(input, &mut encoded, ctx).encode();

    if new_size < input.len() {
        return encoded;
    }
    encoded.clear();
    let _ = NopEncoder::new(input, &mut encoded, ctx).encode();
    encoded
}

/// Try to perform the block decoding, or fall back to the nop decoder.
fn decode_or_nop(input: &[u8]) -> Option<(usize, Vec<u8>)> {
    let mut decoded: Vec<u8> = Vec::new();

    if let Some((read, _)) = BlockDecoder::new(input, &mut decoded).decode() {
        return Some((read, decoded));
    }

    assert_eq!(decoded.len(), 0);
    if let Some((read, _)) = NopDecoder::new(input, &mut decoded).decode() {
        return Some((read, decoded));
    }

    None
}

pub struct FullDecoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
}

impl<'a> Encoder<'a> for FullEncoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>, ctx: Context) -> Self {
        FullEncoder { input, output, ctx }
    }

    fn encode(&mut self) -> usize {
        self.output.extend(FULL_SIG);
        if self.ctx.level == 13 {
            let mut encoder = AAE::new(self.input, self.output, self.ctx);
            return FULL_SIG.len() + encoder.encode();
        }

        let mut encoder = PagerEncoder::new(self.input, self.output, self.ctx);
        encoder.set_callback(encode_or_nop);
        encoder.set_page_size(self.ctx.block_size);
        FULL_SIG.len() + encoder.encode()
    }
}

impl<'a> Decoder<'a> for FullDecoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        FullDecoder { input, output }
    }

    fn decode(&mut self) -> Option<(usize, usize)> {
        if !match_signature(self.input, &FULL_SIG) {
            return None;
        }
        let buffer = &self.input[FULL_SIG.len()..];

        if match_signature(buffer, &ARITH_SIG) {
            let mut decoder = AAD::new(buffer, self.output);
            let (read, written) = decoder.decode()?;
            return Some((read + ARITH_SIG.len() + FULL_SIG.len(), written));
        }

        let mut decoder = PagerDecoder::new(buffer, self.output);
        decoder.set_callback(decode_or_nop);
        let (read, written) = decoder.decode()?;
        Some((read + FULL_SIG.len(), written))
    }
}
