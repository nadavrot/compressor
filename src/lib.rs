pub mod bitvector;
pub mod block;
pub mod coding;
pub mod full;
pub mod lz;
pub mod nop;
pub mod pager;
pub mod utils;

/// Stores information about the environment.
#[derive(Copy, Clone)]
pub struct Context {
    /// Specifies the compression level.
    level: u8,
    /// Specifies the size of each block.
    block_size: usize,
}

impl Context {
    pub fn new(level: u8, block_size: usize) -> Self {
        Self { level, block_size }
    }
}

/// A trait that defines the interface for encoding buffers.
pub trait Encoder<'a> {
    /// Creates a new Encoder that reads from 'input' and writes into 'output',
    /// with the encoder context 'ctx'.
    fn new(input: &'a [u8], output: &'a mut Vec<u8>, ctx: Context) -> Self;

    /// Encode the whole input buffer and return the number of bytes that were
    /// written into the output stream.
    #[must_use]
    fn encode(&mut self) -> usize;
}

/// A trait that defines the interface for decoding buffers.
pub trait Decoder<'a> {
    /// Creates a new Decoder that reads from 'input' and writes into 'output'.
    fn new(input: &'a [u8], output: &'a mut Vec<u8>) -> Self;

    /// Try to decode the buffer 'input', and return the number of input bytes
    /// that were consumed followed by the number of bytes written, or None,
    /// if the input was invalid.
    #[must_use]
    fn decode(&mut self) -> Option<(usize, usize)>;
}
