//! The 'PagerEncoder' and 'PagerDecoder' are responsible for taking a stream of bytes and
//! partitioning them into small blocks that are encoded and decoded individually.

use crate::utils::signatures::{match_signature, PAGER_SIG, START_PAGE_SIG};
use crate::{Context, Decoder, Encoder};

/// A callback for handling the encoding of each block.
pub type EncodeHandlerTy = fn(input: &[u8], ctx: Context) -> Vec<u8>;
/// A callback for handling the decoding of each block.
pub type DecodeHandlerTy = fn(input: &[u8]) -> Option<(usize, Vec<u8>)>;

/// Splits the input stream into segments and encodes each one of them
/// independently using the registered callback.
pub struct PagerEncoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
    /// A callback for encoding each block.
    callback: Option<EncodeHandlerTy>,
    /// Encoder context.
    ctx: Context,
}

impl<'a> PagerEncoder<'a> {
    /// Register a callback for handling each block.
    pub fn set_callback(&mut self, callback: EncodeHandlerTy) {
        self.callback = Some(callback)
    }

    /// Sets the size of each page in the stream.
    pub fn set_page_size(&mut self, new_size: usize) {
        self.ctx.block_size = new_size
    }

    /// Perform the encoding.
    fn encode_impl(&mut self) -> usize {
        let mut parts: Vec<&'a [u8]> = Vec::new();
        assert!(self.ctx.block_size > 0, "Must set page size");

        // Push the parts to process:
        for i in 0..(1 + self.input.len() / self.ctx.block_size) {
            let start = self.ctx.block_size * i;
            let end = (self.ctx.block_size * (i + 1)).min(self.input.len());
            parts.push(&self.input[start..end]);
        }

        // Write the signature.
        self.output.extend(PAGER_SIG);
        let callback = self.callback.unwrap();

        // Compress each one of the pages using the pipeline.
        let mut written = PAGER_SIG.len();
        for part in parts {
            self.output.extend(START_PAGE_SIG);
            let compressed = callback(part, self.ctx);
            self.output.extend(compressed.iter());
            written += START_PAGE_SIG.len() + compressed.len();
        }

        written
    }
}

/// Decodes a stream that was partitioned into multiple pages.
pub struct PagerDecoder<'a> {
    /// The uncompressed input.
    input: &'a [u8],
    /// The output stream.
    output: &'a mut Vec<u8>,
    /// A callback for handling the decoding of each block.
    callback: Option<DecodeHandlerTy>,
}

impl<'a> PagerDecoder<'a> {
    /// Sets the callback for handling the decoding of each block.
    pub fn set_callback(&mut self, callback: DecodeHandlerTy) {
        self.callback = Some(callback)
    }

    /// Decode the input parameter. Returns the number of bytes consumed and the
    /// number of bytes written if the operation succeeded.
    fn decode_impl(&mut self) -> Option<(usize, usize)> {
        if !match_signature(self.input, &PAGER_SIG) {
            return None;
        }
        let callback = self.callback.unwrap();
        let mut cursor = PAGER_SIG.len();
        let mut written = 0;
        while cursor < self.input.len() {
            if !match_signature(&self.input[cursor..], &START_PAGE_SIG) {
                return None;
            }
            cursor += START_PAGE_SIG.len();
            let packet = &self.input[cursor..];
            let (read, buff) = callback(packet)?;
            cursor += read;
            written += buff.len();
            self.output.extend(&buff);
        }
        Some((cursor, written))
    }
}

impl<'a> Encoder<'a> for PagerEncoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>, ctx: Context) -> Self {
        PagerEncoder {
            input,
            output,
            callback: None,
            ctx,
        }
    }

    fn encode(&mut self) -> usize {
        self.encode_impl()
    }
}

impl<'a> Decoder<'a> for PagerDecoder<'a> {
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self {
        PagerDecoder {
            input,
            output,
            callback: None,
        }
    }

    fn decode(&mut self) -> Option<(usize, usize)> {
        self.decode_impl()
    }
}
