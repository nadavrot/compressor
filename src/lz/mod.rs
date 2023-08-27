//! A collection of modules that implement Lempel–Ziv matching.

mod lz4;
pub mod matcher;
pub use lz4::LZ4Decoder;
pub use lz4::LZ4Encoder;
