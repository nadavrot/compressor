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
/// <https://github.com/facebook/zstd/blob/dev/doc/zstd_compression_format.md#offset-codes>
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

/// A lookup table that computes the reciprocal of u16 division.
/// The tables is defined as (1<<32)/i;
pub static RECIPROCAL_U32: [u32; 1024] = [
    0, 0xffffffff, 0x7fffffff, 0x55555555, 0x3fffffff, 0x33333333, 0x2aaaaaaa,
    0x24924924, 0x1fffffff, 0x1c71c71c, 0x19999999, 0x1745d174, 0x15555555,
    0x13b13b13, 0x12492492, 0x11111111, 0xfffffff, 0xf0f0f0f, 0xe38e38e,
    0xd79435e, 0xccccccc, 0xc30c30c, 0xba2e8ba, 0xb21642c, 0xaaaaaaa,
    0xa3d70a3, 0x9d89d89, 0x97b425e, 0x9249249, 0x8d3dcb0, 0x8888888,
    0x8421084, 0x7ffffff, 0x7c1f07c, 0x7878787, 0x7507507, 0x71c71c7,
    0x6eb3e45, 0x6bca1af, 0x6906906, 0x6666666, 0x63e7063, 0x6186186,
    0x5f417d0, 0x5d1745d, 0x5b05b05, 0x590b216, 0x572620a, 0x5555555,
    0x5397829, 0x51eb851, 0x5050505, 0x4ec4ec4, 0x4d4873e, 0x4bda12f,
    0x4a7904a, 0x4924924, 0x47dc11f, 0x469ee58, 0x456c797, 0x4444444,
    0x4325c53, 0x4210842, 0x4104104, 0x3ffffff, 0x3f03f03, 0x3e0f83e,
    0x3d22635, 0x3c3c3c3, 0x3b5cc0e, 0x3a83a83, 0x39b0ad1, 0x38e38e3,
    0x381c0e0, 0x3759f22, 0x369d036, 0x35e50d7, 0x3531dec, 0x3483483,
    0x33d91d2, 0x3333333, 0x329161f, 0x31f3831, 0x3159721, 0x30c30c3,
    0x3030303, 0x2fa0be8, 0x2f14990, 0x2e8ba2e, 0x2e05c0b, 0x2d82d82,
    0x2d02d02, 0x2c8590b, 0x2c0b02c, 0x2b93105, 0x2b1da46, 0x2aaaaaa,
    0x2a3a0fd, 0x29cbc14, 0x295fad4, 0x28f5c28, 0x288df0c, 0x2828282,
    0x27c4597, 0x2762762, 0x2702702, 0x26a439f, 0x2647c69, 0x25ed097,
    0x2593f69, 0x253c825, 0x24e6a17, 0x2492492, 0x243f6f0, 0x23ee08f,
    0x239e0d5, 0x234f72c, 0x2302302, 0x22b63cb, 0x226b902, 0x2222222,
    0x21d9ead, 0x2192e29, 0x214d021, 0x2108421, 0x20c49ba, 0x2082082,
    0x2040810, 0x1ffffff, 0x1fc07f0, 0x1f81f81, 0x1f44659, 0x1f07c1f,
    0x1ecc07b, 0x1e9131a, 0x1e573ac, 0x1e1e1e1, 0x1de5d6e, 0x1dae607,
    0x1d77b65, 0x1d41d41, 0x1d0cb58, 0x1cd8568, 0x1ca4b30, 0x1c71c71,
    0x1c3f8f0, 0x1c0e070, 0x1bdd2b8, 0x1bacf91, 0x1b7d6c3, 0x1b4e81b,
    0x1b20364, 0x1af286b, 0x1ac5701, 0x1a98ef6, 0x1a6d01a, 0x1a41a41,
    0x1a16d3f, 0x19ec8e9, 0x19c2d14, 0x1999999, 0x1970e4f, 0x1948b0f,
    0x1920fb4, 0x18f9c18, 0x18d3018, 0x18acb90, 0x1886e5f, 0x1861861,
    0x183c977, 0x1818181, 0x17f405f, 0x17d05f4, 0x17ad220, 0x178a4c8,
    0x1767dce, 0x1745d17, 0x1724287, 0x1702e05, 0x16e1f76, 0x16c16c1,
    0x16a13cd, 0x1681681, 0x1661ec6, 0x1642c85, 0x1623fa7, 0x1605816,
    0x15e75bb, 0x15c9882, 0x15ac056, 0x158ed23, 0x1571ed3, 0x1555555,
    0x1539094, 0x151d07e, 0x1501501, 0x14e5e0a, 0x14cab88, 0x14afd6a,
    0x149539e, 0x147ae14, 0x1460cbc, 0x1446f86, 0x142d662, 0x1414141,
    0x13fb013, 0x13e22cb, 0x13c995a, 0x13b13b1, 0x13991c2, 0x1381381,
    0x13698df, 0x13521cf, 0x133ae45, 0x1323e34, 0x130d190, 0x12f684b,
    0x12e025c, 0x12c9fb4, 0x12b404a, 0x129e412, 0x1288b01, 0x127350b,
    0x125e227, 0x1249249, 0x1234567, 0x121fb78, 0x120b470, 0x11f7047,
    0x11e2ef3, 0x11cf06a, 0x11bb4a4, 0x11a7b96, 0x1194538, 0x1181181,
    0x116e068, 0x115b1e5, 0x11485f0, 0x1135c81, 0x112358e, 0x1111111,
    0x10fef01, 0x10ecf56, 0x10db20a, 0x10c9714, 0x10b7e6e, 0x10a6810,
    0x10953f3, 0x1084210, 0x1073260, 0x10624dd, 0x105197f, 0x1041041,
    0x103091b, 0x1020408, 0x1010101, 0xffffff, 0xff00ff, 0xfe03f8, 0xfd08e5,
    0xfc0fc0, 0xfb1885, 0xfa232c, 0xf92fb2, 0xf83e0f, 0xf74e3f, 0xf6603d,
    0xf57403, 0xf4898d, 0xf3a0d5, 0xf2b9d6, 0xf1d48b, 0xf0f0f0, 0xf00f00,
    0xef2eb7, 0xee500e, 0xed7303, 0xec9791, 0xebbdb2, 0xeae564, 0xea0ea0,
    0xe93965, 0xe865ac, 0xe79372, 0xe6c2b4, 0xe5f36c, 0xe52598, 0xe45932,
    0xe38e38, 0xe2c4a6, 0xe1fc78, 0xe135a9, 0xe07038, 0xdfac1f, 0xdee95c,
    0xde27eb, 0xdd67c8, 0xdca8f1, 0xdbeb61, 0xdb2f17, 0xda740d, 0xd9ba42,
    0xd901b2, 0xd84a59, 0xd79435, 0xd6df43, 0xd62b80, 0xd578e9, 0xd4c77b,
    0xd41732, 0xd3680d, 0xd2ba08, 0xd20d20, 0xd16154, 0xd0b69f, 0xd00d00,
    0xcf6474, 0xcebcf8, 0xce168a, 0xcd7127, 0xcccccc, 0xcc2978, 0xcb8727,
    0xcae5d8, 0xca4587, 0xc9a633, 0xc907da, 0xc86a78, 0xc7ce0c, 0xc73293,
    0xc6980c, 0xc5fe74, 0xc565c8, 0xc4ce07, 0xc4372f, 0xc3a13d, 0xc30c30,
    0xc27806, 0xc1e4bb, 0xc15250, 0xc0c0c0, 0xc0300c, 0xbfa02f, 0xbf112a,
    0xbe82fa, 0xbdf59c, 0xbd6910, 0xbcdd53, 0xbc5264, 0xbbc840, 0xbb3ee7,
    0xbab656, 0xba2e8b, 0xb9a786, 0xb92143, 0xb89bc3, 0xb81702, 0xb79300,
    0xb70fbb, 0xb68d31, 0xb60b60, 0xb58a48, 0xb509e6, 0xb48a39, 0xb40b40,
    0xb38cf9, 0xb30f63, 0xb2927c, 0xb21642, 0xb19ab5, 0xb11fd3, 0xb0a59b,
    0xb02c0b, 0xafb321, 0xaf3add, 0xaec33e, 0xae4c41, 0xadd5e6, 0xad602b,
    0xaceb0f, 0xac7691, 0xac02b0, 0xab8f69, 0xab1cbd, 0xaaaaaa, 0xaa392f,
    0xa9c84a, 0xa957fa, 0xa8e83f, 0xa87917, 0xa80a80, 0xa79c7b, 0xa72f05,
    0xa6c21d, 0xa655c4, 0xa5e9f6, 0xa57eb5, 0xa513fd, 0xa4a9cf, 0xa44029,
    0xa3d70a, 0xa36e71, 0xa3065e, 0xa29ecf, 0xa237c3, 0xa1d139, 0xa16b31,
    0xa105a9, 0xa0a0a0, 0xa03c16, 0x9fd809, 0x9f747a, 0x9f1165, 0x9eaecc,
    0x9e4cad, 0x9deb06, 0x9d89d8, 0x9d2921, 0x9cc8e1, 0x9c6916, 0x9c09c0,
    0x9baade, 0x9b4c6f, 0x9aee72, 0x9a90e7, 0x9a33cd, 0x99d722, 0x997ae7,
    0x991f1a, 0x98c3ba, 0x9868c8, 0x980e41, 0x97b425, 0x975a75, 0x97012e,
    0x96a850, 0x964fda, 0x95f7cc, 0x95a025, 0x9548e4, 0x94f209, 0x949b92,
    0x944580, 0x93efd1, 0x939a85, 0x93459b, 0x92f113, 0x929ceb, 0x924924,
    0x91f5bc, 0x91a2b3, 0x915009, 0x90fdbc, 0x90abcc, 0x905a38, 0x900900,
    0x8fb823, 0x8f67a1, 0x8f1779, 0x8ec7ab, 0x8e7835, 0x8e2917, 0x8dda52,
    0x8d8be3, 0x8d3dcb, 0x8cf008, 0x8ca29c, 0x8c5584, 0x8c08c0, 0x8bbc50,
    0x8b7034, 0x8b246a, 0x8ad8f2, 0x8a8dcd, 0x8a42f8, 0x89f874, 0x89ae40,
    0x89645c, 0x891ac7, 0x88d180, 0x888888, 0x883fdd, 0x87f780, 0x87af6f,
    0x8767ab, 0x872032, 0x86d905, 0x869222, 0x864b8a, 0x86053c, 0x85bf37,
    0x85797b, 0x853408, 0x84eedd, 0x84a9f9, 0x84655d, 0x842108, 0x83dcf9,
    0x839930, 0x8355ac, 0x83126e, 0x82cf75, 0x828cbf, 0x824a4e, 0x820820,
    0x81c635, 0x81848d, 0x814327, 0x810204, 0x80c121, 0x808080, 0x804020,
    0x7fffff, 0x7fc01f, 0x7f807f, 0x7f411e, 0x7f01fc, 0x7ec318, 0x7e8472,
    0x7e460a, 0x7e07e0, 0x7dc9f3, 0x7d8c42, 0x7d4ece, 0x7d1196, 0x7cd49a,
    0x7c97d9, 0x7c5b53, 0x7c1f07, 0x7be2f6, 0x7ba71f, 0x7b6b82, 0x7b301e,
    0x7af4f3, 0x7aba01, 0x7a7f48, 0x7a44c6, 0x7a0a7c, 0x79d06a, 0x79968f,
    0x795ceb, 0x79237d, 0x78ea45, 0x78b144, 0x787878, 0x783fe1, 0x780780,
    0x77cf53, 0x77975b, 0x775f97, 0x772807, 0x76f0aa, 0x76b981, 0x76828b,
    0x764bc8, 0x761537, 0x75ded9, 0x75a8ac, 0x7572b2, 0x753ce8, 0x750750,
    0x74d1e9, 0x749cb2, 0x7467ac, 0x7432d6, 0x73fe30, 0x73c9b9, 0x739572,
    0x73615a, 0x732d70, 0x72f9b6, 0x72c62a, 0x7292cc, 0x725f9b, 0x722c99,
    0x71f9c4, 0x71c71c, 0x7194a1, 0x716253, 0x713031, 0x70fe3c, 0x70cc72,
    0x709ad4, 0x706962, 0x70381c, 0x700700, 0x6fd60f, 0x6fa549, 0x6f74ae,
    0x6f443c, 0x6f13f5, 0x6ee3d8, 0x6eb3e4, 0x6e8419, 0x6e5478, 0x6e2500,
    0x6df5b0, 0x6dc68a, 0x6d978b, 0x6d68b5, 0x6d3a06, 0x6d0b80, 0x6cdd21,
    0x6caee9, 0x6c80d9, 0x6c52ef, 0x6c252c, 0x6bf790, 0x6bca1a, 0x6b9ccb,
    0x6b6fa1, 0x6b429e, 0x6b15c0, 0x6ae907, 0x6abc74, 0x6a9006, 0x6a63bd,
    0x6a3799, 0x6a0b99, 0x69dfbd, 0x69b406, 0x698873, 0x695d04, 0x6931b8,
    0x690690, 0x68db8b, 0x68b0aa, 0x6885eb, 0x685b4f, 0x6830d6, 0x680680,
    0x67dc4c, 0x67b23a, 0x67884a, 0x675e7c, 0x6734d0, 0x670b45, 0x66e1db,
    0x66b893, 0x668f6c, 0x666666, 0x663d80, 0x6614bc, 0x65ec17, 0x65c393,
    0x659b30, 0x6572ec, 0x654ac8, 0x6522c3, 0x64fadf, 0x64d319, 0x64ab74,
    0x6483ed, 0x645c85, 0x64353c, 0x640e11, 0x63e706, 0x63c018, 0x639949,
    0x637299, 0x634c06, 0x632591, 0x62ff3a, 0x62d900, 0x62b2e4, 0x628ce5,
    0x626703, 0x62413f, 0x621b97, 0x61f60d, 0x61d09e, 0x61ab4d, 0x618618,
    0x6160ff, 0x613c03, 0x611722, 0x60f25d, 0x60cdb5, 0x60a928, 0x6084b6,
    0x606060, 0x603c25, 0x601806, 0x5ff401, 0x5fd017, 0x5fac49, 0x5f8895,
    0x5f64fb, 0x5f417d, 0x5f1e18, 0x5eface, 0x5ed79e, 0x5eb488, 0x5e918c,
    0x5e6ea9, 0x5e4be1, 0x5e2932, 0x5e069c, 0x5de420, 0x5dc1bd, 0x5d9f73,
    0x5d7d42, 0x5d5b2b, 0x5d392c, 0x5d1745, 0x5cf578, 0x5cd3c3, 0x5cb226,
    0x5c90a1, 0x5c6f35, 0x5c4de1, 0x5c2ca5, 0x5c0b81, 0x5bea75, 0x5bc980,
    0x5ba8a3, 0x5b87dd, 0x5b672f, 0x5b4698, 0x5b2618, 0x5b05b0, 0x5ae55e,
    0x5ac524, 0x5aa500, 0x5a84f3, 0x5a64fc, 0x5a451c, 0x5a2553, 0x5a05a0,
    0x59e603, 0x59c67c, 0x59a70c, 0x5987b1, 0x59686c, 0x59493e, 0x592a24,
    0x590b21, 0x58ec33, 0x58cd5a, 0x58ae97, 0x588fe9, 0x587151, 0x5852cd,
    0x58345f, 0x581605, 0x57f7c0, 0x57d990, 0x57bb75, 0x579d6e, 0x577f7c,
    0x57619f, 0x5743d5, 0x572620, 0x57087f, 0x56eaf3, 0x56cd7a, 0x56b015,
    0x5692c4, 0x567587, 0x56585e, 0x563b48, 0x561e46, 0x560158, 0x55e47c,
    0x55c7b4, 0x55ab00, 0x558e5e, 0x5571d0, 0x555555, 0x5538ed, 0x551c97,
    0x550055, 0x54e425, 0x54c807, 0x54abfd, 0x549005, 0x54741f, 0x54584c,
    0x543c8b, 0x5420dc, 0x540540, 0x53e9b5, 0x53ce3d, 0x53b2d7, 0x539782,
    0x537c3f, 0x53610e, 0x5345ef, 0x532ae2, 0x530fe6, 0x52f4fb, 0x52da22,
    0x52bf5a, 0x52a4a3, 0x5289fe, 0x526f6a, 0x5254e7, 0x523a75, 0x522014,
    0x5205c4, 0x51eb85, 0x51d156, 0x51b738, 0x519d2b, 0x51832f, 0x516943,
    0x514f67, 0x51359c, 0x511be1, 0x510237, 0x50e89c, 0x50cf12, 0x50b598,
    0x509c2e, 0x5082d4, 0x50698a, 0x505050, 0x503725, 0x501e0b, 0x500500,
    0x4fec04, 0x4fd319, 0x4fba3d, 0x4fa170, 0x4f88b2, 0x4f7004, 0x4f5766,
    0x4f3ed6, 0x4f2656, 0x4f0de5, 0x4ef583, 0x4edd30, 0x4ec4ec, 0x4eacb7,
    0x4e9490, 0x4e7c79, 0x4e6470, 0x4e4c76, 0x4e348b, 0x4e1cae, 0x4e04e0,
    0x4ded20, 0x4dd56f, 0x4dbdcc, 0x4da637, 0x4d8eb1, 0x4d7739, 0x4d5fcf,
    0x4d4873, 0x4d3126, 0x4d19e6, 0x4d02b5, 0x4ceb91, 0x4cd47b, 0x4cbd73,
    0x4ca679, 0x4c8f8d, 0x4c78ae, 0x4c61dd, 0x4c4b19, 0x4c3464, 0x4c1dbb,
    0x4c0720, 0x4bf093, 0x4bda12, 0x4bc3a0, 0x4bad3a, 0x4b96e2, 0x4b8097,
    0x4b6a58, 0x4b5428, 0x4b3e04, 0x4b27ed, 0x4b11e3, 0x4afbe6, 0x4ae5f6,
    0x4ad012, 0x4aba3c, 0x4aa472, 0x4a8eb5, 0x4a7904, 0x4a6360, 0x4a4dc9,
    0x4a383e, 0x4a22c0, 0x4a0d4e, 0x49f7e8, 0x49e28f, 0x49cd42, 0x49b802,
    0x49a2cd, 0x498da5, 0x497889, 0x496379, 0x494e75, 0x49397e, 0x492492,
    0x490fb2, 0x48fade, 0x48e616, 0x48d159, 0x48bca9, 0x48a804, 0x48936b,
    0x487ede, 0x486a5c, 0x4855e6, 0x48417b, 0x482d1c, 0x4818c8, 0x480480,
    0x47f043, 0x47dc11, 0x47c7eb, 0x47b3d0, 0x479fc1, 0x478bbc, 0x4777c3,
    0x4763d5, 0x474ff2, 0x473c1a, 0x47284d, 0x47148b, 0x4700d5, 0x46ed29,
    0x46d987, 0x46c5f1, 0x46b266, 0x469ee5, 0x468b6f, 0x467804, 0x4664a3,
    0x46514e, 0x463e02, 0x462ac2, 0x46178b, 0x460460, 0x45f13f, 0x45de28,
    0x45cb1c, 0x45b81a, 0x45a522, 0x459235, 0x457f52, 0x456c79, 0x4559aa,
    0x4546e6, 0x45342c, 0x45217c, 0x450ed6, 0x44fc3a, 0x44e9a8, 0x44d720,
    0x44c4a2, 0x44b22e, 0x449fc3, 0x448d63, 0x447b0d, 0x4468c0, 0x44567d,
    0x444444, 0x443214, 0x441fee, 0x440dd2, 0x43fbc0, 0x43e9b7, 0x43d7b7,
    0x43c5c2, 0x43b3d5, 0x43a1f2, 0x439019, 0x437e49, 0x436c82, 0x435ac5,
    0x434911, 0x433766, 0x4325c5, 0x43142d, 0x43029e, 0x42f118, 0x42df9b,
    0x42ce28, 0x42bcbd, 0x42ab5c, 0x429a04, 0x4288b4, 0x42776e, 0x426631,
    0x4254fc, 0x4243d1, 0x4232ae, 0x422195, 0x421084, 0x41ff7c, 0x41ee7c,
    0x41dd86, 0x41cc98, 0x41bbb2, 0x41aad6, 0x419a02, 0x418937, 0x417874,
    0x4167ba, 0x415708, 0x41465f, 0x4135bf, 0x412527, 0x411497, 0x410410,
    0x40f391, 0x40e31a, 0x40d2ac, 0x40c246, 0x40b1e9, 0x40a193, 0x409146,
    0x408102, 0x4070c5, 0x406090, 0x405064, 0x404040, 0x403024, 0x402010,
    0x401004,
];
