use crate::models::dmc::DMCModel;
use crate::models::Model;

use crate::utils::signatures::{match_signature, ARITH_SIG};
use crate::utils::signatures::{read32, write32};
use crate::{Context, Decoder, Encoder};

use super::arithmetic::{BitonicDecoder, BitonicEncoder};

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
        let mut model = DMCModel::new();

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
        let mut model = DMCModel::new();

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
