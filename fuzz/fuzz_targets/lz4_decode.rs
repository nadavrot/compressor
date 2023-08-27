#![no_main]

use compressor::lz::LZ4Decoder;
use compressor::Decoder;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut decompressed: Vec<u8> = Vec::new();
    {
        let mut decoder = LZ4Decoder::new(data, &mut decompressed);
        let _ = decoder.decode();
    }
});
