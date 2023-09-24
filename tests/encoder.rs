use compressor::coding::entropy::{EntropyDecoder, EntropyEncoder};
use compressor::Encoder;
use compressor::{Context, Decoder};
use rand_distr::Distribution;

type EncoderTy<'a> = EntropyEncoder<'a, 256, 4096>;
type DecoderTy<'a> = EntropyDecoder<'a, 256, 4096>;

#[test]
fn test_round_trip_simple_encoder() {
    let text = "entropy encoding is typically the last stage of a compression pipeline";
    let text = text.as_bytes();
    let mut compressed: Vec<u8> = Vec::new();
    let mut decompressed: Vec<u8> = Vec::new();
    let ctx = Context::new(9, 1 << 20);

    // Define an encoder with 8bit symbols, and 12bit states.
    let _ = EncoderTy::new(text, &mut compressed, ctx).encode();
    let _ = DecoderTy::new(&compressed, &mut decompressed).decode();

    println!("Decoded {:?}", decompressed);
    println!("Input length = {}", 8 * text.len());
    println!("Compressed length = {}", compressed.len());
    assert_eq!(text, decompressed);
}

#[allow(dead_code)]
fn round_trip(input: &[u8]) {
    let ctx = Context::new(9, 1 << 20);

    let mut compressed = Vec::new();
    // Define an encoder with 8bit symbols, and 12bit states.
    let mut enc = EncoderTy::new(input, &mut compressed, ctx);
    let compressed_size = enc.encode();
    assert_eq!(compressed.len(), compressed_size);

    let mut decompressed = Vec::new();
    let mut decoder =
        EntropyDecoder::<256, 4096>::new(&compressed, &mut decompressed);
    let (consumed, decompressed_size) = decoder.decode().unwrap();
    assert_eq!(compressed.len(), consumed);
    assert_eq!(decompressed.len(), decompressed_size);
    assert_eq!(decompressed, input);
}

#[test]
fn test_simple_coder_round_trip() {
    round_trip(&[]);
    round_trip(&[0]);
    round_trip(&[0, 0, 0, 0]);
    round_trip(&[0, 0, 1, 1, 2, 3]);
    round_trip(&[1, 251, 255, 0, 245, 32, 32, 142]);
    round_trip(&[254, 254, 254, 0, 0]);

    round_trip(&[
        254, 254, 254, 0, 0, 1, 251, 255, 0, 245, 32, 32, 142, 1, 251, 255, 0,
        245, 32, 32, 142, 38, 10, 223, 223, 102, 38, 10, 223, 223, 102, 99, 99,
    ]);

    round_trip(&[
        38, 10, 223, 223, 102, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 223, 223, 102,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 48, 254, 59, 36, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 48, 254, 59, 36,
    ]);
}

#[test]
fn test_simple_decoder() {
    let mut output = Vec::new();
    let _ = DecoderTy::new(&[], &mut output).decode();
    let _ = DecoderTy::new(&[1], &mut output).decode();
    let _ = DecoderTy::new(&vec![1; 1000], &mut output).decode();
    let _ = DecoderTy::new(&vec![0; 10000], &mut output).decode();
}

#[test]
fn test_simple_decoder_random() {
    use rand::thread_rng;
    use rand_distr::Uniform;

    let mut rng = thread_rng();
    let distr = Uniform::new(0, 255);

    // Do this number of attempts to decode the random input.
    for i in 1..80 {
        let mut input = Vec::new();
        // Generate large buffers to decode.
        for _ in 0..i * 1001 {
            input.push(distr.sample(&mut rng) as u8);
        }

        let mut decoded = Vec::new();
        if let Some((read, written)) =
            DecoderTy::new(&input, &mut decoded).decode()
        {
            assert_eq!(read, input.len());
            assert_eq!(written, decoded.len());
        }
    }
}
