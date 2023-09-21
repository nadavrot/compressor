//! A collection of utilities for handling arrays, numbers and signatures.

/// A collection of signatures for the different encoders.
pub mod signatures {
    /// Signatures for different encoding kinds.
    pub const LZ4_SIG: [u8; 4] = [0x17, 0x41, 0x74, 0x17];
    pub const NOP_ENC: [u8; 2] = [0x90, 0x90];
    pub const SIMPLE_ENC: [u8; 2] = [0x12, 34];
    pub const BLOCK_SIG: [u8; 2] = [0x13, 45];
    pub const ARITH_SIG: [u8; 2] = [0x01, 10];
    pub const PAGER_SIG: [u8; 4] = [0x9a, 0x93, 0x9a, 0x93];
    pub const START_PAGE_SIG: [u8; 2] = [0x71, 75];
    pub const FULL_SIG: [u8; 4] = [0x10, 0x14, 0x82, 0x35];
    pub const FILE_EXTENSION: &str = ".rz";

    /// Return True if 'input' starts with 'signature'.
    pub fn match_signature(input: &[u8], signature: &[u8]) -> bool {
        input.starts_with(signature)
    }

    /// Write the value 'val' into 'stream'.
    pub fn write32(val: u32, stream: &mut Vec<u8>) {
        let bytes = val.to_be_bytes();
        stream.extend(bytes);
    }

    /// Try to decode a number from the input buffer.
    pub fn read32(input: &[u8]) -> Option<u32> {
        if input.len() < 4 {
            return None;
        }
        if let Ok(x) = input[0..4].try_into() {
            return Some(u32::from_be_bytes(x));
        }
        None
    }
}

/// Implements run length encoding.
pub mod run_length_encoding {
    use super::number_encoding;

    pub struct RLEIterator<'a> {
        input: &'a [u8],
        prev: u32,
        len: usize,
        pos: usize,
    }

    impl<'a> RLEIterator<'a> {
        pub fn new(input: &'a [u8]) -> Self {
            Self {
                input,
                prev: 0xffffffff,
                len: 0,
                pos: 0,
            }
        }
    }

    impl<'a> Iterator for RLEIterator<'a> {
        type Item = (u8, usize);

        fn next(&mut self) -> Option<Self::Item> {
            while self.pos < self.input.len() {
                if self.prev != self.input[self.pos] as u32 {
                    let item = (self.prev as u8, self.len);
                    self.prev = self.input[self.pos] as u32;
                    self.len = 1;
                    self.pos += 1;
                    return Some(item);
                }
                self.pos += 1;
                self.len += 1;
            }
            if self.len > 0 {
                let item = (self.prev as u8, self.len);
                self.len = 0;
                return Some(item);
            }

            None
        }
    }

    // A helper function for writing an RLE chunk into a stream.
    fn write_rle(mut len: usize, val: u8, stream: &mut Vec<u8>) -> usize {
        let mut wrote = 0;
        while len > 255 {
            stream.push(255);
            stream.push(val);
            len -= 255;
            wrote += 2;
        }
        if len > 0 {
            stream.push(len as u8);
            stream.push(val);
            wrote += 2;
        }
        wrote
    }

    // Encode the array and return the number of bytes written.
    pub fn encode(array: &[u8], stream: &mut Vec<u8>) -> usize {
        number_encoding::encode32(array.len() as u32, stream);
        let mut wrote = 4;
        for item in RLEIterator::new(array) {
            wrote += write_rle(item.1, item.0, stream);
        }
        wrote
    }

    // Decode the array and return the number of items that were read.
    pub fn decode(input: &[u8], output: &mut Vec<u8>) -> Option<usize> {
        let array_len = input.len();
        let (_, len) = number_encoding::decode32(input)?;
        let len = len as usize;
        let mut wrote = 0;
        let mut pos = 4;
        while wrote < len {
            if pos + 1 >= array_len {
                return None;
            }
            let rl = input[pos];
            let val = input[pos + 1];
            for _ in 0..rl {
                output.push(val);
            }
            pos += 2;
            wrote += rl as usize;
        }
        Some(pos)
    }
}

/// Implement encoding and decoding of variable length integers.
/// The number is written as a u8 byte. If the number is greater than 0xff then
/// 0xff is written and the remaining of the number is serialized onwards.
/// The number 350 is serialized as [255, 95].
pub mod variable_length_encoding {
    use super::number_encoding;

    /// Encode the number 'num' into the stream and return the number of bytes
    /// written.
    pub fn encode(num: u32, stream: &mut Vec<u8>) -> usize {
        let mut val = num;
        let mut written = 0;
        while val >= 255 {
            written += 1;
            val -= 255;
            stream.push(255);
        }
        written += 1;
        stream.push((val & 0xff) as u8);
        written
    }

    /// Decode a number from the stream and return the number of bytes written
    /// and the value that was loaded.
    pub fn decode(stream: &[u8]) -> Option<(usize, u32)> {
        let len = stream.len();

        let mut val: u32 = 0;
        let mut read = 0;

        // Decode a number.
        loop {
            // Check for overflow while decoding number.
            if read >= len {
                return None;
            }
            let c = stream[read];
            read += 1;
            val += c as u32;

            if c != 255 {
                break;
            }
        }
        Some((read, val))
    }

    // Encode the array and return the number of bytes written.
    pub fn encode_array32(array: &[u32], stream: &mut Vec<u8>) -> usize {
        number_encoding::encode32(array.len() as u32, stream);
        let mut written = 4;
        for num in array {
            written += encode(*num, stream);
        }
        written
    }

    // Decode the array and return the number of items that were read.
    pub fn decode_array32(
        stream: &[u8],
        array: &mut Vec<u32>,
    ) -> Option<usize> {
        let (_, len) = number_encoding::decode32(stream)?;
        let mut cursor = 4;
        for _ in 0..len {
            let (read, val) = decode(&stream[cursor..])?;
            cursor += read;
            array.push(val);
        }
        Some(cursor)
    }
}

/// Implements encoding and decoding of regular numbers.
pub mod number_encoding {
    pub fn encode32(num: u32, stream: &mut Vec<u8>) -> usize {
        stream.extend_from_slice(&(num).to_be_bytes());
        4
    }

    pub fn decode32(stream: &[u8]) -> Option<(usize, u32)> {
        if stream.len() < 4 {
            return None;
        }

        let bytes: [u8; 4] = stream[0..4].try_into().unwrap();
        Some((4, u32::from_be_bytes(bytes)))
    }

    pub fn encode16(num: u16, stream: &mut Vec<u8>) -> usize {
        stream.extend_from_slice(&(num).to_be_bytes());
        2
    }

    pub fn decode16(stream: &[u8]) -> Option<(usize, u16)> {
        if stream.len() < 2 {
            return None;
        }

        let bytes: [u8; 2] = stream[0..2].try_into().unwrap();
        Some((2, u16::from_be_bytes(bytes)))
    }

    // Encode the array and return the number of bytes written.
    pub fn encode_array16(array: &[u16], stream: &mut Vec<u8>) -> usize {
        encode32(array.len() as u32, stream);
        let mut written = 4;
        for num in array {
            written += encode16(*num, stream);
        }
        written
    }

    // Decode the array and return the number of items that were read.
    pub fn decode_array16(
        stream: &[u8],
        array: &mut Vec<u16>,
    ) -> Option<usize> {
        let (_, len) = decode32(stream)?;
        let mut cursor = 4;
        for _ in 0..len {
            let (read, val) = decode16(&stream[cursor..])?;
            cursor += read;
            array.push(val);
        }
        Some(cursor)
    }
}

/// Implements encoding and decoding of arrays.
pub mod array_encoding {
    use super::number_encoding;

    // Encode the array and return the number of bytes written.
    pub fn encode(array: &[u8], stream: &mut Vec<u8>) -> usize {
        number_encoding::encode32(array.len() as u32, stream);
        stream.extend_from_slice(array);
        array.len() + 4
    }

    // Decode the array and return the number of items that were read.
    pub fn decode(stream: &[u8], array: &mut Vec<u8>) -> Option<usize> {
        let (_, len) = number_encoding::decode32(stream)?;
        let len = len as usize;
        if stream[4..].len() < len {
            return None;
        }
        array.extend(&stream[4..len + 4]);
        Some(4 + len)
    }
}

/// Encodes numbers into two streams: tokens and extra bits. This is useful when
/// there is a sharp distribution of values, with few high-bit numbers.
/// The first stream stores state values in the range 0..N, and the second
/// stream stores the extra bits. The representation of the value is
/// (1 << code) + read_bits(code). The numbers are shifted by +1, to allow the
/// encoding of zero. This encoder encodes the range [0 .. u32::MAX-1].
/// Reference:
/// https://github.com/facebook/zstd/blob/dev/doc/zstd_compression_format.md#offset-codes
pub mod two_stream_encoding {
    use super::number_encoding;
    use crate::bitvector::Bitvector;

    /// Encode 'val' into a token, and stores the extra bits into 'bv'.
    pub fn encode32(val: u32, bv: &mut Bitvector) -> u32 {
        let code = 32 - (val + 1).leading_zeros() - 1;
        bv.push_word((val + 1) as u64, code as usize);
        code
    }

    /// Decode a value from the token, and extracts the extra bits from 'bv'.
    pub fn decode32(code: u32, bv: &mut Bitvector) -> u32 {
        (1 << code) + bv.pop_word(code as usize) as u32 - 1
    }

    #[test]
    fn test_two_stream_encoding_simple() {
        let mut bv = Bitvector::new();
        let val = 7;
        let tok = encode32(val, &mut bv);
        let res = decode32(tok, &mut bv);
        assert_eq!(bv.len(), 0);
        assert_eq!(val, res);
    }

    #[test]
    fn test_two_stream_encoding_many() {
        for i in 0..130 {
            let mut bv = Bitvector::new();
            let tok = encode32(i, &mut bv);
            let res = decode32(tok, &mut bv);
            assert_eq!(bv.len(), 0);
            assert_eq!(i, res);
            assert!(tok < 8);
        }
    }

    #[test]
    fn test_two_stream_encoding_tokens() {
        let mut bv = Bitvector::new();
        let vals = [
            0, 1, 2, 3, 5, 16, 37, 1121, 3512, 17824, 69481, 32768, 41910,
            65535, 65536, 65537, 192151,
        ];
        for val in vals {
            let tok = encode32(val, &mut bv);
            let res = decode32(tok, &mut bv);
            assert_eq!(bv.len(), 0);
            assert_eq!(val, res);
        }
    }

    /// Encode the array and return the number of bytes written to the stream.
    pub fn encode_array32(
        array: &[u32],
        stream: &mut Vec<u8>,
        bv: &mut Bitvector,
    ) -> usize {
        let written = number_encoding::encode32(array.len() as u32, stream);
        for val in array {
            stream.push(encode32(*val, bv) as u8);
        }
        written + array.len()
    }

    /// Decode the array and return the number of items that were read.
    pub fn decode_array32(
        stream: &[u8],
        array: &mut Vec<u32>,
        bv: &mut Bitvector,
    ) -> Option<usize> {
        // We need to process the values in reverse, because the bits are
        // stored in the bitvector in reverse.
        let mut res = Vec::new();
        let (read, len) = number_encoding::decode32(stream)?;
        let len = len as usize;
        for i in 0..len {
            res.push(decode32(stream[read + (len - i - 1)] as u32, bv));
        }
        res.reverse();
        array.extend(res);
        Some(read + len)
    }

    #[test]
    fn test_two_stream_encoding_array() {
        let vals = [
            16, 37, 1121, 3512, 17824, 69481, 32768, 41910, 65535, 65536,
            65537, 192151,
        ];

        let mut encoded = Vec::new();
        let mut decoded = Vec::new();

        let mut bv = Bitvector::new();
        let written = encode_array32(&vals, &mut encoded, &mut bv);
        let read = decode_array32(&encoded, &mut decoded, &mut bv).unwrap();

        assert_eq!(written, encoded.len());
        assert_eq!(read, encoded.len());
        assert_eq!(bv.len(), 0);
        assert_eq!(vals.to_vec(), decoded);
    }
}
