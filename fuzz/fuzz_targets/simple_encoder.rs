#![no_main]

use compressor::coding::simple::{SimpleDecoder, SimpleEncoder};
use compressor::{Decoder, Encoder};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut compressed = Vec::new();
    let mut decompressed = Vec::new();

    let written =
        SimpleEncoder::<256, 4096>::new(data, &mut compressed).encode();
    assert_eq!(written, compressed.len());

    let (read, written) =
        SimpleDecoder::<256, 4096>::new(&compressed, &mut decompressed)
            .decode()
            .unwrap();
    assert_eq!(decompressed, data);
    assert_eq!(read, compressed.len());
    assert_eq!(written, decompressed.len());
});
