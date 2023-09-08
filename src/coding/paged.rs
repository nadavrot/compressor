//! A module that implements a simple tANS entropy encoder.

use super::simple::{SimpleDecoder, SimpleEncoder};
use crate::utils::signatures::{match_signature, PAGER_SIG, START_PAGE_SIG};
use crate::{Context, Decoder, Encoder};

pub struct PagedEntropyEncoder<
    'a,
    const ALPHABET: usize,
    const TABLESIZE: usize,
    const PAGESIZE: usize,
> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
    /// The compression context (unused).
    ctx: Context,
}

impl<
        'a,
        const ALPHABET: usize,
        const TABLESIZE: usize,
        const PAGESIZE: usize,
    > Encoder<'a> for PagedEntropyEncoder<'a, ALPHABET, TABLESIZE, PAGESIZE>
{
    fn new(input: &'a [u8], output: &'a mut Vec<u8>, ctx: Context) -> Self {
        PagedEntropyEncoder { input, output, ctx }
    }

    fn encode(&mut self) -> usize {
        // Write the signature.
        self.output.extend(PAGER_SIG);
        let mut wrote = PAGER_SIG.len();

        // For each one of the segments:
        for i in 0..(1 + self.input.len() / PAGESIZE) {
            let start = PAGESIZE * i;
            let end = (PAGESIZE * (i + 1)).min(self.input.len());
            let page = &self.input[start..end];
            // Write the signature and the encoded segment.
            self.output.extend(START_PAGE_SIG);
            wrote += START_PAGE_SIG.len();
            wrote += SimpleEncoder::<ALPHABET, TABLESIZE>::new(
                page,
                self.output,
                self.ctx,
            )
            .encode();
        }
        wrote
    }
}

pub struct PagedEntropyDecoder<
    'a,
    const ALPHABET: usize,
    const TABLESIZE: usize,
> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
}

impl<'a, const ALPHABET: usize, const TABLESIZE: usize> Decoder<'a>
    for PagedEntropyDecoder<'a, ALPHABET, TABLESIZE>
{
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        PagedEntropyDecoder { input, output }
    }

    fn decode(&mut self) -> Option<(usize, usize)> {
        if !match_signature(self.input, &PAGER_SIG) {
            return None;
        }
        let mut cursor = PAGER_SIG.len();
        let mut written = 0;
        while cursor < self.input.len() {
            // Read the page signature.
            if !match_signature(&self.input[cursor..], &START_PAGE_SIG) {
                return None;
            }
            cursor += START_PAGE_SIG.len();
            // Figure out where the data starts.
            let packet = &self.input[cursor..];

            // Have the decoder decode a single packet.
            let (read, wrote) =
                SimpleDecoder::<ALPHABET, TABLESIZE>::new(packet, self.output)
                    .decode()?;
            cursor += read;
            written += wrote;
        }
        Some((cursor, written))
    }
}
