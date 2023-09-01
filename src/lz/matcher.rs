//! This module implements a reusable Lempel–Ziv matcher.

use std::ops::Range;

/// Controls the size, and depth of the dictionary.
const DICTIONARY_SIZE_BITS: usize = 13;
const DICTIONARY_SIZE: usize = 1 << DICTIONARY_SIZE_BITS;
/// Controls the number of ways in the cache.
const DICTIONARY_BANKS: usize = 8;
/// Used to mark empty cells.
const EMPTY_CELL: u32 = 0xffffffff;

// Enable a form of non-greedy parsing. This is explained here:
// http://fastcompression.blogspot.com/2011/12/advanced-parsing-strategies.html
const ENABLE_OPT_PARSE: bool = true;
// The number of bytes forward that the program can search. This is a number
// between 1 and 4.
const PARSE_MAX_SEARCH: usize = 4;

// The minimum size of the match word.
const MIN_MATCH: usize = 4;

/// A Lempel–Ziv based matcher.
struct LzDictionary<'a, const MAX_OFFSET: usize, const MAX_MATCH: usize> {
    /// The input to tokenize.
    input: &'a [u8],
    /// Maps a sequence of bytes to their index in the sequence.
    /// The match could be a hash collision or an uninitialized value.
    /// Matches may reside in one of the rotating LRU banks.
    dict: Box<[u32; DICTIONARY_SIZE * DICTIONARY_BANKS]>,
}

impl<'a, const MAX_OFFSET: usize, const MAX_MATCH: usize>
    LzDictionary<'a, MAX_OFFSET, MAX_MATCH>
{
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            dict: Box::from([EMPTY_CELL; DICTIONARY_SIZE * DICTIONARY_BANKS]),
        }
    }

    /// Returns the length of the input string
    pub fn len(&self) -> usize {
        self.input.len()
    }

    fn get_bytes_at(&self, idx: usize) -> u32 {
        let val: [u8; 4] =
            self.input[idx..idx + 4].try_into().expect("Out of bounds");
        u32::from_ne_bytes(val)
    }

    fn hash_to_index(val: u32) -> usize {
        let val = val.wrapping_mul(0x797124e5);
        let val = val >> (32 - DICTIONARY_SIZE_BITS);
        val as usize
    }

    /// Return True if we can prove that this match is not longer than the best
    /// match.
    fn early_disqualify(&self, a: usize, b: usize, best_size: usize) -> bool {
        debug_assert!(a < b, "Pointer b must come after pointer a");
        b + best_size < self.input.len()
            && self.input[a + best_size] != self.input[b + best_size]
    }

    /// Return the size of a string that starts at 'a' and 'b' indices.
    /// The index 'a' must come before 'b'.
    fn get_match_length(&self, a: usize, b: usize) -> usize {
        let mut len = 0;
        let size = self.input.len();
        let mut a = a;
        let mut b = b;
        debug_assert!(a < b, "Pointer b must come after pointer a");

        if self.input[a..a + 4] == self.input[b..b + 4] {
            a += 4;
            b += 4;
            len += 4;
        }

        let end = size.min(b + MAX_MATCH - 4);
        while b < end && self.input[a] == self.input[b] {
            a += 1;
            b += 1;
            len += 1;
        }
        len
    }

    /// Return a match to a previous string that matches a string that starts at
    /// 'idx'.
    fn get_match(&self, idx: usize) -> Option<Range<usize>> {
        let dic_idx = self.get_match_candidate(idx);
        let mut best = 0..0;

        for i in 0..DICTIONARY_BANKS {
            let loc = self.dict[dic_idx * DICTIONARY_BANKS + i];
            // Ignore empty cells.
            if loc == EMPTY_CELL {
                break;
            }
            // Ignore match distances that are too big.
            let offset = idx - loc as usize;
            if offset >= MAX_OFFSET {
                break;
            }
            if self.early_disqualify(loc as usize, idx, best.len()) {
                continue;
            }
            let len = self.get_match_length(loc as usize, idx);
            if best.len() < len {
                best = (loc as usize)..(loc as usize) + len;
            }
        }

        if best.len() >= 4 {
            Some(best)
        } else {
            None
        }
    }

    /// Return a possible match candidate for a string that starts at 'idx'.
    fn get_match_candidate(&self, idx: usize) -> usize {
        Self::hash_to_index(self.get_bytes_at(idx))
    }
    /// Save the value at index 'idx' to the LRU dictionary.
    fn save_match(&mut self, idx: usize) {
        let dic_idx = self.get_match_candidate(idx);
        // This is an LRU cache. Move the old entries to make room to the new
        // entry.
        let base = dic_idx * DICTIONARY_BANKS;
        for i in (0..DICTIONARY_BANKS - 1).rev() {
            self.dict[base + (i + 1)] = self.dict[base + (i)];
        }
        self.dict[base] = idx as u32;
    }

    /// Grow the match region backwards into the literal section.
    /// This is necessary because an earlier match may fail because
    /// of a hash collision or a match that's too short.
    /// Returns the number of bytes that can be removed from the literal region.
    fn grow_match_backwards(
        &self,
        lit: &Range<usize>,
        mat: &Range<usize>,
    ) -> usize {
        // We go lit_len steps backwards, so don't overflow. Also, don't handle
        // empty match or literal packets.
        if mat.start <= lit.len() || mat.is_empty() || lit.is_empty() {
            return 0;
        }
        let mut match_ptr = mat.start - 1;
        let mut lit_ptr = lit.start + lit.len() - 1;
        let mut i = 0;

        while self.input[match_ptr] == self.input[lit_ptr] && i < lit.len() {
            match_ptr -= 1;
            lit_ptr -= 1;
            i += 1;
        }
        i
    }
}

/// A Lempel–Ziv based matcher.
pub struct Matcher<'a, const MAX_OFFSET: usize, const MAX_MATCH: usize> {
    /// The input to tokenize.
    dict: LzDictionary<'a, MAX_OFFSET, MAX_MATCH>,
    /// The iterator location in the input.
    cursor: usize,
}

impl<'a, const MAX_OFFSET: usize, const MAX_MATCH: usize>
    Matcher<'a, MAX_OFFSET, MAX_MATCH>
{
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            dict: LzDictionary::new(input),
            cursor: 0,
        }
    }

    /// Return the next literal and match regions, which could be empty.
    /// The indices in the regions are absolute from the beginning of the
    /// stream.
    fn get_next_match_region(
        &mut self,
    ) -> Option<(Range<usize>, Range<usize>)> {
        // Grow the literal section, and on each step look for a previous match.
        let mut lit = self.cursor..self.cursor;
        let input_len = self.dict.len();
        if self.cursor == input_len {
            return None;
        }

        // For each character in the input buffer:
        'outer: while self.cursor + MIN_MATCH < input_len {
            // Check if there is a previous match, and save the hash.
            let mat = self.dict.get_match(self.cursor);
            self.dict.save_match(self.cursor);

            if let Some(mut mat) = mat {
                // If we found a match, try to see if one of the next chars is a
                // better candidate.
                let end_of_buffer = self.cursor + MIN_MATCH * 2 > input_len;
                if ENABLE_OPT_PARSE && !end_of_buffer {
                    for i in 1..PARSE_MAX_SEARCH {
                        if let Some(mat2) = self.dict.get_match(self.cursor + i)
                        {
                            // If we skip one char we might find a better match!
                            if mat2.len() >= mat.len() + i {
                                self.cursor += i;
                                lit = lit.start..lit.end + i;
                                continue 'outer;
                            }
                        }
                    }
                }

                // Try to increase the size of the match backwards and take from
                // the literals.
                let reduce = self.dict.grow_match_backwards(&lit, &mat);

                // Insert all of the hashes in the input into the dictionary.
                // Don't insert the next value because we don't want to have it
                // in the dictionary when we do the next iteration (hence the -1).
                let start = self.cursor + 1;
                let stop = (start + mat.len()).min(input_len - MIN_MATCH) - 1;
                for i in start..stop {
                    self.dict.save_match(i);
                }

                // Update the cursor and return the match.
                self.cursor += mat.len();
                mat = mat.start - reduce..mat.end;
                lit = lit.start..lit.end - reduce;
                return Some((lit, mat));
            }

            // We didn't find a match. Grow the literal region and move on.
            self.cursor += 1;
            lit = lit.start..lit.end + 1;
        }

        // We are close to the end of the buffer. Grow the literal section.
        while self.cursor < input_len {
            self.cursor += 1;
            lit = lit.start..lit.end + 1;
        }

        Some((lit, 0..0))
    }

    pub fn iter(&'a mut self) -> MatchIterator<'a, MAX_OFFSET, MAX_MATCH> {
        MatchIterator { matcher: self }
    }
}

pub struct MatchIterator<'a, const MAX_OFFSET: usize, const MAX_MATCH: usize> {
    matcher: &'a mut Matcher<'a, MAX_OFFSET, MAX_MATCH>,
}

impl<'a, const MAX_OFFSET: usize, const MAX_MATCH: usize> Iterator
    for MatchIterator<'a, MAX_OFFSET, MAX_MATCH>
{
    type Item = (Range<usize>, Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        self.matcher.get_next_match_region()
    }
}
