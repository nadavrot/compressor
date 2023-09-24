#![no_main]

use compressor::coding::entropy::EntropyEncoder;
use compressor::Decoder;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut output = Vec::new();
    let _ = EntropyEncoder::<256, 512>::new(data, &mut output).decode();
});
