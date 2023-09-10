use compressor::block::{decode_offset_stream, encode_offset_stream};
use compressor::block::{BlockDecoder, BlockEncoder};
use compressor::full::{FullDecoder, FullEncoder};
use compressor::pager::{PagerDecoder, PagerEncoder};
use compressor::{Context, Decoder, Encoder};

#[test]
fn test_block_round_trip() {
    fn round_trip(input: &[u8]) {
        let mut compressed: Vec<u8> = Vec::new();
        let ctx = Context::new(9, 1 << 20);
        {
            let mut encoder = BlockEncoder::new(input, &mut compressed, ctx);
            let written = encoder.encode();
            assert_eq!(written, compressed.len());
        }

        let mut decompressed: Vec<u8> = Vec::new();
        {
            let mut decoder = BlockDecoder::new(&compressed, &mut decompressed);
            let (consumed, written) = decoder.decode().unwrap();
            assert_eq!(consumed, compressed.len());
            assert_eq!(written, input.len());
        }
        assert_eq!(decompressed, input);
    }

    {
        round_trip(&[]);
        round_trip(&[1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2, 0, 0, 0, 0]);
        round_trip(&[1, 0, 0, 1, 0, 0, 0, 0, 2, 2, 2, 2, 0, 0, 0, 0]);
        round_trip(&[1, 1, 1, 1, 1, 1, 1, 1, 1, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    }
    {
        let test_str = "123456789abcd&ef&gh&ijk&lm7no*aaaa aaaa aaaaaaaa";
        let input = test_str.as_bytes();
        round_trip(input);
    }

    {
        let mut vec = Vec::new();
        for i in 0..10 {
            for j in 0..10 {
                vec.push(i);
                vec.push(j);
                vec.push(j * 2);
            }
        }
        round_trip(&vec);
    }
}

#[test]
fn test_full_round_trip() {
    fn round_trip(input: &[u8]) {
        let mut compressed: Vec<u8> = Vec::new();
        let ctx = Context::new(9, 1 << 10);

        {
            let mut encoder = FullEncoder::new(input, &mut compressed, ctx);
            let written = encoder.encode();
            assert_eq!(written, compressed.len());
        }

        let mut decompressed: Vec<u8> = Vec::new();
        {
            let mut decoder = FullDecoder::new(&compressed, &mut decompressed);
            let (consumed, written) = decoder.decode().unwrap();
            assert_eq!(consumed, compressed.len());
            assert_eq!(written, input.len());
        }
        assert_eq!(decompressed, input);
    }

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
        let mut vec = Vec::new();
        for i in 0..10 {
            for j in 0..10 {
                for k in 0..10 {
                    vec.push(i + (j ^ k));
                }
            }
        }
        round_trip(&vec);
    }
}

#[test]
fn test_pager_round_trip() {
    fn encode_nop(input: &[u8], ctx: Context) -> Vec<u8> {
        use compressor::nop::NopEncoder;
        let mut encoded: Vec<u8> = Vec::new();
        let _ = NopEncoder::new(input, &mut encoded, ctx).encode();
        encoded
    }

    fn decode_nop(input: &[u8]) -> Option<(usize, Vec<u8>)> {
        use compressor::nop::NopDecoder;
        let mut decoded: Vec<u8> = Vec::new();
        if let Some((read, _)) = NopDecoder::new(input, &mut decoded).decode() {
            return Some((read, decoded));
        }

        None
    }

    fn round_trip(input: &[u8]) {
        let mut compressed: Vec<u8> = Vec::new();
        let ctx = Context::new(9, 0);

        {
            let mut encoder = PagerEncoder::new(input, &mut compressed, ctx);
            encoder.set_callback(encode_nop);
            encoder.set_page_size(15);
            let written = encoder.encode();
            assert_eq!(written, compressed.len());
        }

        let mut decompressed: Vec<u8> = Vec::new();
        {
            let mut decoder = PagerDecoder::new(&compressed, &mut decompressed);
            decoder.set_callback(decode_nop);
            let (consumed, written) = decoder.decode().unwrap();
            assert_eq!(consumed, compressed.len());
            assert_eq!(written, input.len());
        }
        assert_eq!(decompressed, input);
    }

    {
        round_trip(&[]);
        round_trip(&[1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2]);
        round_trip(&[1, 1, 1, 1, 0, 0, 0, 0, 2, 2, 2, 2, 0, 0, 0, 0]);
    }
    {
        round_trip(&[1, 1, 1, 1, 1, 1, 1, 1, 1, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    }
    {
        let mut vec = Vec::new();
        for i in 0..10 {
            for j in 0..10 {
                for k in 0..10 {
                    vec.push(i + (j ^ k));
                }
            }
        }
        round_trip(&vec);
    }
}

#[test]
fn test_offset_encoder() {
    let input = [0, 1, 2, 3, 12, 65233, 11241];
    let ctx = Context::new(5, 120);
    let res = encode_offset_stream::<17>(&input, ctx);
    let out = decode_offset_stream::<17>(&res).unwrap();
    assert_eq!(out, input);
}
