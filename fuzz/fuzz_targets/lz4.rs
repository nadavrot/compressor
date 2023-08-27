#![no_main]

use compressor::lz::{LZ4Decoder, LZ4Encoder};
use compressor::{Decoder, Encoder};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut compressed: Vec<u8> = Vec::new();

    {
        let mut encoder = LZ4Encoder::new(data, &mut compressed);
        let written = encoder.encode();
        assert_eq!(written, compressed.len());
    }

    let mut decompressed: Vec<u8> = Vec::new();
    {
        let mut decoder = LZ4Decoder::new(&compressed, &mut decompressed);
        let (consumed, written) = decoder.decode().unwrap();
        assert_eq!(consumed, compressed.len());
        assert_eq!(written, decompressed.len());
    }
    assert_eq!(decompressed, data);
});
