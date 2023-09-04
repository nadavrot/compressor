//! An implementation of a general bit vector that allows pushing and popping of
//! bits.

#[derive(PartialEq, Debug)]
pub struct Bitvector {
    /// Stores the packed part of the bitvector.
    data: Vec<u64>,
    /// Stores the last 64bit vectors, for easy access.
    /// The bits are always packed to the right [xxxxx012345]
    /// The last word always has 0..63 bits.
    last: u64,
    // Points to the next free bit (also size of bitvector).
    len: usize,
}

impl Default for Bitvector {
    fn default() -> Self {
        Self::new()
    }
}

impl Bitvector {
    pub fn new() -> Bitvector {
        Bitvector {
            data: Vec::new(),
            last: 0,
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.last = 0;
        self.data = Vec::new();
    }

    /// Set all of the bits above 'keep' to zero.
    pub fn clear_upper_bits(bits: u64, keep: usize) -> u64 {
        let amt: u32 = (64 - keep) as u32;
        let shl = bits.checked_shl(amt).unwrap_or(0);
        shl.checked_shr(amt).unwrap_or(0)
    }

    /// Push the lowest 'len' bits from 'bits'.
    pub fn push_word(&mut self, bits: u64, len: usize) {
        debug_assert!(len <= 64, "Pushing too many bits");
        let bits = Self::clear_upper_bits(bits, len);
        let avail = 64 - self.len % 64;

        // Try to push the bits into the free word.
        if avail >= len {
            self.last <<= len % 64;
            self.last |= bits;
            self.len += len;

            // If the free word is filled, flush it.
            if self.len % 64 == 0 && len > 0 {
                self.data.push(self.last);
                self.last = 0;
            }
            return;
        }

        // Push the first chunk:
        let upper = bits >> (len - avail);
        let lower = Self::clear_upper_bits(bits, len - avail);
        self.last <<= avail; // Make room for 'upper'
        self.last |= upper; // Fill the bits.

        // Save the free word and start a new word.
        self.data.push(self.last);
        self.last = lower;
        self.len += len;
    }

    /// Remove 'len' bits from 'bits'.
    #[must_use]
    pub fn pop_word(&mut self, len: usize) -> u64 {
        debug_assert!(self.len >= len, "Taking too many bits");
        let avail = self.len % 64;

        // Try to extract the bits from the last word.
        if avail >= len {
            let curr = self.last;
            self.last >>= len;
            self.len -= len;
            return Self::clear_upper_bits(curr, len);
        }

        // We need to take some bits from the free word and some from the
        // next word. The upper part of the word sit in the bit stream and
        // the lower part sits in the free word.
        // [XXXXXXXX UUUUU][.....LL]

        self.len -= len; // Mark the bits as taken.
        let low_len = avail; // Take all of the available bits.
        let high_len = len - avail; // Update how many bits are left to take.

        let lower = Self::clear_upper_bits(self.last, low_len);

        // Next, take the next few bits from the next word. Notice that we need
        // to take at least one bit to satisfy the requirement that the last
        // word as 0..63 bits.
        self.last = self.data.pop().unwrap();
        let upper = Self::clear_upper_bits(self.last, high_len);
        self.last >>= high_len % 64;
        (upper << low_len) | lower
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dump(&self) {
        print!("{{");
        for elem in self.data.iter() {
            print!("{:b}, ", elem);
        }
        print!("}}");
        print!("{{");
        print!("{:b}, ", self.last);
        println!("}}");
    }

    /// Save the bitvector to a stream of bytes. Report the number of bytes
    /// written.
    pub fn serialize(&self, output: &mut Vec<u8>) -> usize {
        // Write the length field.
        output.extend_from_slice(&(self.len as u32).to_be_bytes());
        // Write the free word part.
        output.extend_from_slice(&(self.last).to_be_bytes());
        // Write the packed part.
        for elem in &self.data {
            output.extend_from_slice(&elem.to_be_bytes());
        }

        4 + (self.data.len() + 1) * 8
    }

    /// Load the bit-vector from a stream of bytes. Returns the bitvector and
    /// the number of bytes that were read.
    pub fn deserialize(input: &[u8]) -> Option<(Self, usize)> {
        if input.len() < 4 {
            return None;
        }
        // Read the length.
        let bytes: [u8; 4] = input[0..4].try_into().unwrap();
        let length_field = u32::from_be_bytes(bytes) as usize;
        let input: &[u8] = &input[4..];

        // Read the free word.
        let bytes: [u8; 8] = input[0..8].try_into().unwrap();
        let last: u64 = u64::from_be_bytes(bytes);
        let input: &[u8] = &input[8..];

        // Read the packed payload.
        let mut payload: Vec<u64> = Vec::new();
        let len = input.len();
        let available_u64s = len / 8;

        // Check that we have enough data in the buffer to fill the bitvector.
        if length_field >= (1 + available_u64s) * 64 {
            return None;
        }

        // The unit of idx is 'u8'. We count the number of bytes that were
        // deserialized into u8.
        let mut idx = 0;
        let mut len_to_read = length_field;
        while len_to_read >= 64 {
            let bytes: [u8; 8] = input[idx..idx + 8].try_into().unwrap();
            payload.push(u64::from_be_bytes(bytes));
            idx += 8;
            len_to_read -= 64;
        }

        Some((
            Bitvector {
                data: payload,
                len: length_field,
                last,
            },
            idx + 12,
        ))
    }
}
