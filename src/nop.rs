//! This is a simple encoder that serializes the input and does not change it.

use crate::utils::number_encoding::decode32;
use crate::utils::number_encoding::encode32;
use crate::utils::signatures::{match_signature, NOP_ENC};
use crate::{Decoder, Encoder};
pub struct NopEncoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
}

impl<'a> NopEncoder<'a> {
    pub fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        Self { input, output }
    }

    fn encode_impl(&mut self) -> usize {
        self.output.extend(NOP_ENC);
        encode32(self.input.len() as u32, self.output);
        self.output.extend(self.input);
        // Bytes written plus the signature.
        NOP_ENC.len() + 4 + self.input.len()
    }
}

pub struct NopDecoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
}

impl<'a> NopDecoder<'a> {
    pub fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        Self { input, output }
    }

    fn decode_impl(&mut self) -> Option<(usize, usize)> {
        let sig_len = NOP_ENC.len();
        if !match_signature(self.input, &NOP_ENC) {
            return None;
        }
        let (_, buff_len) = decode32(&self.input[sig_len..])?;
        let start = sig_len + 4;
        self.output
            .extend(&self.input[start..start + buff_len as usize]);
        Some((start + buff_len as usize, buff_len as usize))
    }
}

impl<'a> Encoder<'a> for NopEncoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        NopEncoder { input, output }
    }

    fn encode(&mut self) -> usize {
        self.encode_impl()
    }
}

impl<'a> Decoder<'a> for NopDecoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        NopDecoder { input, output }
    }

    fn decode(&mut self) -> Option<(usize, usize)> {
        self.decode_impl()
    }
}
