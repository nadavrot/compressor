use compressor::lz::{LZ4Decoder, LZ4Encoder};
use compressor::{Context, Decoder, Encoder};

const INPUT0_PLAIN: [u8; 63] = [
    0x74, 0x68, 0x69, 0x73, 0x20, 0x69, 0x73, 0x20, 0x61, 0x20, 0x74, 0x65,
    0x73, 0x74, 0x20, 0xa, 0x74, 0x68, 0x69, 0x73, 0x20, 0x69, 0x73, 0x20,
    0x61, 0x20, 0x74, 0x65, 0x73, 0x74, 0x20, 0xa, 0x74, 0x68, 0x69, 0x73,
    0x20, 0x69, 0x73, 0x20, 0x61, 0x20, 0x73, 0x69, 0x6d, 0x70, 0x6c, 0x65,
    0x20, 0x74, 0x65, 0x73, 0x74, 0x20, 0x66, 0x6f, 0x72, 0x20, 0x6c, 0x7a,
    0x34, 0x20, 0xa,
];

/// The content of INPUT0_PLAIN compressed with the lz4 compressor.
const INPUT0_COMPRESSED: [u8; 40] = [
    0xff, 0x1, 0x74, 0x68, 0x69, 0x73, 0x20, 0x69, 0x73, 0x20, 0x61, 0x20,
    0x74, 0x65, 0x73, 0x74, 0x20, 0xa, 0x10, 0x0, 0x7, 0x62, 0x73, 0x69, 0x6d,
    0x70, 0x6c, 0x65, 0x17, 0x0, 0x90, 0x66, 0x6f, 0x72, 0x20, 0x6c, 0x7a,
    0x34, 0x20, 0xa,
];

#[test]
fn test_lz4_decoder_const_inputs() {
    {
        let mut stream: Vec<u8> = Vec::new();
        let mut decoder = LZ4Decoder::new(&INPUT0_COMPRESSED, &mut stream);
        let res = decoder.decode();
        assert!(res.is_some());
        assert_eq!(INPUT0_PLAIN[..], stream);
    }
}

fn round_trip(input: &[u8]) {
    for level in [1, 3, 4, 9] {
        let mut compressed: Vec<u8> = Vec::new();
        let ctx = Context::new(level, 1 << 20);

        {
            let mut encoder = LZ4Encoder::new(input, &mut compressed, ctx);
            let written = encoder.encode();
            assert_eq!(written, compressed.len());
        }

        let mut decompressed: Vec<u8> = Vec::new();
        {
            let mut decoder = LZ4Decoder::new(&compressed, &mut decompressed);
            let (consumed, written) = decoder.decode().unwrap();
            assert_eq!(consumed, compressed.len());
            assert_eq!(written, input.len());
        }
        assert_eq!(decompressed, input);
    }
}

#[test]
fn test_lz4_encoder_decoder_cons_inputs() {
    {
        round_trip(&[]);
        round_trip(&[
            1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 1, 1,
            1, 1,
        ]);
        round_trip(&[1, 1, 1, 1, 0, 0, 0, 0, 2, 2, 2, 2, 0, 0, 0, 0]);
    }
    {
        round_trip(&[1, 1, 1, 1, 1, 1, 1, 1, 1, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    }
    {
        let test_str = "123456789abcdefghijklmno*aaaa aaaa aaaaaaaa";
        let input = test_str.as_bytes();
        round_trip(input);
    }
    {
        let test_str = "
        0152340 c1bc 0012 0000 0000 0100 0000 0000 0000
        0152350 0065 0000 0000 0000 b763 0012 0000 0000
        0152360 0200 0000 0000 0000 0066 0000 0000 0000
        0152370 c1a8 0012 0000 0000 0400 0000 0000 0000
        0152380 0000 0000 0000 0000 cb54 0012 0000 0000
        0152390 1000 0000 0000 0000 0000 0000 0000 0000
        01523a0 aa6d 0012 0000 0000 2000 0000 0000 0000
        01523b0 0000 0000 0000 0000 cb5e 0012 0000 0000
        01523c0 0800 0000 0000 0000 0067 0000 0000 0000
        01523d0 ae89 0012 0000 0000 4000 0000 0000 0000";
        let input = test_str.as_bytes();
        round_trip(input);
    }
}

#[test]
fn test_lz4_decoder_no_crash() {
    {
        let mut stream: Vec<u8> = Vec::new();
        let mut decoder = LZ4Decoder::new(&[], &mut stream);
        let _ = decoder.decode();
    }

    {
        let mut stream: Vec<u8> = Vec::new();
        let mut decoder = LZ4Decoder::new(&[46, 12], &mut stream);
        let _ = decoder.decode();
    }

    {
        let mut stream: Vec<u8> = Vec::new();
        let mut decoder = LZ4Decoder::new(&[10, 10, 15], &mut stream);
        let _ = decoder.decode();
    }
}

#[test]
fn test_lz4_encoder_const_inputs() {
    let ctx = Context::new(9, 1 << 20);
    let mut stream: Vec<u8> = Vec::new();
    let mut encoder = LZ4Encoder::new(&INPUT0_PLAIN, &mut stream, ctx);
    let written = encoder.encode();
    assert_eq!(stream, INPUT0_COMPRESSED);
    assert_eq!(stream.len(), written);
}
