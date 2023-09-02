use compressor::nop::{NopDecoder, NopEncoder};
use compressor::{Context, Decoder, Encoder};

fn round_trip(input: &[u8]) {
    let mut compressed: Vec<u8> = Vec::new();
    let ctx = Context::new(9, 0);

    {
        let mut encoder = NopEncoder::new(input, &mut compressed, ctx);
        let written = encoder.encode();
        assert_eq!(written, compressed.len());
    }

    let mut decompressed: Vec<u8> = Vec::new();
    {
        let mut decoder = NopDecoder::new(&compressed, &mut decompressed);
        let (consumed, written) = decoder.decode().unwrap();
        assert_eq!(consumed, compressed.len());
        assert_eq!(written, input.len());
    }
    assert_eq!(decompressed, input);
}

#[test]
fn test_nop_round_trip() {
    round_trip(&[]);
    round_trip(&[1, 1]);
    round_trip(&[1, 2, 3, 1, 0, 0, 0, 0, 2, 2, 2, 2, 0, 0, 0]);
}
