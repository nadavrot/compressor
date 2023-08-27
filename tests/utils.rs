use compressor::utils::run_length_encoding;

#[test]
fn test_var_len_encoding_test() {
    use compressor::utils::number_encoding::{decode_array16, encode_array16};
    use compressor::utils::variable_length_encoding::decode_array32;
    use compressor::utils::variable_length_encoding::encode_array32;

    fn test_round_trip16(input: &[u16]) {
        let mut buffer0 = Vec::new();
        let mut buffer1 = Vec::new();

        let wrote = encode_array16(input, &mut buffer0);
        assert_eq!(buffer0.len(), wrote);
        let read = decode_array16(&buffer0, &mut buffer1).unwrap();
        assert_eq!(wrote, read);
        assert_eq!(input, buffer1);
    }

    test_round_trip16(&[]);
    test_round_trip16(&[0]);
    test_round_trip16(&[1, 2, 3]);
    test_round_trip16(&[65535]);
    test_round_trip16(&[10, 65535, 12, 121, 1982, 11111]);

    fn test_round_trip32(input: &[u32]) {
        let mut buffer0 = Vec::new();
        let mut buffer1 = Vec::new();

        let wrote = encode_array32(input, &mut buffer0);
        assert_eq!(buffer0.len(), wrote);
        let read = decode_array32(&buffer0, &mut buffer1).unwrap();
        assert_eq!(wrote, read);
        assert_eq!(input, buffer1);
    }

    test_round_trip32(&[]);
    test_round_trip32(&[0]);
    test_round_trip32(&[1, 2, 3]);
    test_round_trip32(&[1, 65536, 1982, 1 << 20]);
    test_round_trip32(&[1, 2, 255, 256, 65536, 1 << 12, 256, 0, (1 << 18) - 1]);
}

#[test]
fn test_rle() {
    let input = &[99, 99, 99, 103, 104, 105, 79, 79, 79];
    let expected = &[0, 0, 0, 9, 3, 99, 1, 103, 1, 104, 1, 105, 3, 79];

    let mut buff = Vec::new();
    run_length_encoding::encode(input, &mut buff);
    assert_eq!(&buff, expected);

    let mut buff = Vec::new();
    run_length_encoding::decode(expected, &mut buff);
    assert_eq!(&buff, input);
}
