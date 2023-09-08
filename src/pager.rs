//! The 'PagerEncoder' and 'PagerDecoder' are responsible for taking a stream of bytes and
//! partitioning them into small blocks that are encoded and decoded individually.

use crate::utils::signatures::{
    match_signature, read32, write32, PAGER_SIG, START_PAGE_SIG,
};
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

        let callback = self.callback.unwrap();

        // Write the signature and the number of parts.
        self.output.extend(PAGER_SIG);
        write32(parts.len() as u32, self.output);
        let mut written = PAGER_SIG.len() + 4;

        // Compress each one of the pages using the pipeline.
        for part in parts {
            self.output.extend(START_PAGE_SIG);
            let compressed = callback(part, self.ctx);
            self.output.extend((compressed.len() as u32).to_be_bytes());
            self.output.extend(compressed.iter());
            written += START_PAGE_SIG.len() + 4 + compressed.len();
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
        let callback = self.callback.unwrap();
        if !match_signature(self.input, &PAGER_SIG) {
            return None;
        }
        let mut cursor = PAGER_SIG.len();
        let parts = read32(&self.input[cursor..])?;
        cursor += 4;

        let mut written = 0;
        for _ in 0..parts {
            // Read the part signature.
            if !match_signature(&self.input[cursor..], &START_PAGE_SIG) {
                return None;
            }
            cursor += START_PAGE_SIG.len();

            // Read the part length.
            let length = read32(&self.input[cursor..])? as usize;
            cursor += 4;

            let packet = &self.input[cursor..cursor + length as usize];
            let (read, buff) = callback(packet)?;
            debug_assert_eq!(read, length, "Invalid packet?");

            cursor += length;
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
