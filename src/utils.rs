//! A collection of utilities for handling arrays, numbers and signatures.

/// A collection of signatures for the different encoders.
pub mod signatures {
    /// Signatures for different encoding kinds.
    pub const LZ4_SIG: [u8; 4] = [0x17, 0x41, 0x74, 0x17];
    pub const NOP_ENC: [u8; 2] = [0x90, 90];
    pub const SIMPLE_ENC: [u8; 2] = [0x12, 34];
    pub const BLOCK_SIG: [u8; 2] = [0x13, 45];
    pub const PAGER_SIG: [u8; 4] = [0x9a, 0x93, 0x9a, 0x93];
    pub const FULL_SIG: [u8; 4] = [0x10, 0x14, 0x82, 0x35];
    pub const FILE_EXTENSION: &str = ".rz";

    /// Return True if 'input' starts with 'signature'.
    pub fn match_signature(input: &[u8], signature: &[u8]) -> bool {
        input.starts_with(signature)
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
