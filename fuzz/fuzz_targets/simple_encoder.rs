#![no_main]

use compressor::coding::entropy::{EntropyDecoder, EntropyEncoder};
use compressor::{Context, Decoder, Encoder};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut compressed = Vec::new();
    let mut decompressed = Vec::new();
    let ctx = Context::new(9, 1 << 20);

    let written =
        EntropyEncoder::<256, 4096>::new(data, &mut compressed, ctx).encode();
    assert_eq!(written, compressed.len());

    let (read, written) =
        EntropyDecoder::<256, 4096>::new(&compressed, &mut decompressed)
            .decode()
            .unwrap();
    assert_eq!(decompressed, data);
    assert_eq!(read, compressed.len());
    assert_eq!(written, decompressed.len());
});
