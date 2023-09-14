//! An implementation of a general bit vector that allows pushing and popping of
//! bits.

#[derive(PartialEq, Debug)]
pub struct Bitvector {
    /// Stores the packed part of the bitvector.
    data: Vec<u64>,
    /// Stores the last 64bit vectors, for easy access.
    /// The bits are always packed to the right [xxxxx543210]
    /// The last word always has 0..63 bits. Bits above 'len' bits are zero.
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

    pub fn verify(&self) {
        // Check the invariant that the bits trailing the last allocated bit are
        // all zero.
        debug_assert!(
            self.last == Self::clear_upper_bits(self.last, self.len % 64)
        );
        // Check that we have the right number of words allocated to support the
        // length of the bitstream.
        let allocated = self.data.len() * 64 + 64;
        debug_assert!(self.len < allocated && self.len + 64 >= allocated);
    }

    /// Set all of the bits above 'keep' to zero.
    pub fn clear_upper_bits(bits: u64, keep: usize) -> u64 {
        if keep == 0 {
            return 0;
        }
        let amt: u32 = (64 - keep) as u32;
        let shl = bits.overflowing_shl(amt).0;
        shl.overflowing_shr(amt).0
    }

    /// Push the lowest 'len' bits from 'bits'. The bits are inserted into the
    /// bitstream from the right as if shifted right one by one.
    pub fn push_word(&mut self, bits: u64, num: usize) {
        debug_assert!(num <= 64, "Pushing too many bits");
        let bits = Self::clear_upper_bits(bits, num);
        let first_free_bit = self.len % 64;
        let avail = 64 - first_free_bit;

        // Try to push the bits into the free word.
        if avail >= num {
            self.last |= bits << first_free_bit;
            self.len += num;

            // If the free word is filled, flush it.
            if self.len % 64 == 0 && num > 0 {
                self.data.push(self.last);
                self.last = 0;
            }
            self.verify();
            return;
        }

        // Prepare the upper part of the word that does not fit in the current
        // free word. It will go into a new free word.
        let upper_part = Self::clear_upper_bits(bits >> avail, num - avail);

        // Save save the lower part of the input to the upper part of the free
        // word and save it to the stream.
        self.last |= bits << first_free_bit;
        self.data.push(self.last);

        self.last = upper_part;
        self.len += num;
        self.verify();
    }

    /// Remove 'len' bits from 'bits'.
    #[must_use]
    pub fn pop_word(&mut self, num: usize) -> u64 {
        debug_assert!(self.len >= num, "Taking too many bits");
        let avail = self.len % 64;

        // Try to extract the bits from the last word.
        if avail >= num {
            let res = self.last >> (avail - num);
            self.last = Self::clear_upper_bits(self.last, avail - num);
            self.len -= num;
            self.verify();
            return res;
        }

        // We need to take some bits from the free word and some from the
        // next word. The upper part of the word sit in the free word and
        // the lower part sits in the allocated array.
        // [XXXXXXXX LLLLL][UUUU....]

        self.len -= num; // Mark the bits as taken.
        let upper_len = avail; // Prepare to take all bits from the free word.
        let lower_len = num - avail; // Find out how many more bits are needed.

        let upper = Self::clear_upper_bits(self.last, upper_len);

        // Next, take the next few bits from the next word. Notice that we need
        // to take at least one bit to satisfy the requirement that the last
        // word as 0..63 bits.
        self.last = self.data.pop().unwrap();
        // Take the upper part of the next word.
        let lower = self.last >> (64 - lower_len);
        // Overwrite it with zeros to ensure that bits beyond the bitstream are
        // always zero.
        self.last = Self::clear_upper_bits(self.last, self.len % 64);
        self.verify();
        (upper << (lower_len % 64)) | lower
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
