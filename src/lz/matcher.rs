//! This module implements a reusable Lempel–Ziv matcher.
use std::ops::Range;

/// Used to mark empty cells.
const EMPTY_CELL: u32 = 0xffffffff;
// The minimum size of the match word.
const MIN_MATCH: usize = 4;

/// A Lempel–Ziv based matcher. Parameters:
/// MAX_OFFSET controls the maximum size of match offset.
/// MAX_MATCH controls the maximum length of matches.
/// DICT_SIZE_BITS Controls the size of the cache (1<<x).
/// DICT_BANKS number of ways in the LRU cache.
/// PARSE_SEARCH controls the look ahead scan of the matcher (1..4).
struct LzDictionary<
    'a,
    const MAX_OFFSET: usize,
    const MAX_MATCH: usize,
    const DICT_SIZE_BITS: usize,
    const DICT_BANKS: usize,
> {
    /// The input to tokenize.
    input: &'a [u8],
    /// Maps a sequence of bytes to their index in the sequence.
    /// The match could be a hash collision or an uninitialized value.
    /// Matches may reside in one of the rotating LRU banks.
    dict: Vec<u32>,
}

impl<
        'a,
        const MAX_OFFSET: usize,
        const MAX_MATCH: usize,
        const DICT_SIZE_BITS: usize,
        const DICT_BANKS: usize,
    > LzDictionary<'a, MAX_OFFSET, MAX_MATCH, DICT_SIZE_BITS, DICT_BANKS>
{
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            dict: vec![EMPTY_CELL; (1 << DICT_SIZE_BITS) * DICT_BANKS],
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
        let val = val >> (32 - DICT_SIZE_BITS);
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
    fn get_match(&self, idx: usize) -> Range<usize> {
        let dic_idx = self.get_match_candidate(idx);
        let mut best = 0..0;

        for i in 0..DICT_BANKS {
            let loc = self.dict[dic_idx * DICT_BANKS + i];
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

        if best.len() >= MIN_MATCH {
            best
        } else {
            0..0
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
        let base = dic_idx * DICT_BANKS;
        for i in (0..DICT_BANKS - 1).rev() {
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

/// A Lempel–Ziv based matcher. It performs parsing with a lookahead window of
/// 'PARSE_SEARCH' items.
pub struct Matcher<
    'a,
    const MAX_OFFSET: usize,
    const MAX_MATCH: usize,
    const DICT_SIZE_BITS: usize,
    const DICT_BANKS: usize,
    const PARSE_SEARCH: usize,
> {
    /// The input to tokenize.
    dict: LzDictionary<'a, MAX_OFFSET, MAX_MATCH, DICT_SIZE_BITS, DICT_BANKS>,
    /// The iterator location in the input.
    cursor: usize,
}

impl<
        'a,
        const MAX_OFFSET: usize,
        const MAX_MATCH: usize,
        const DICT_SIZE_BITS: usize,
        const DICT_BANKS: usize,
        const PARSE_SEARCH: usize,
    >
    Matcher<'a, MAX_OFFSET, MAX_MATCH, DICT_SIZE_BITS, DICT_BANKS, PARSE_SEARCH>
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

            if !mat.is_empty() {
                // If we found a match, try to see if one of the next chars is a
                // better candidate.
                let end_of_buffer = self.cursor + MIN_MATCH * 2 > input_len;
                if !end_of_buffer {
                    // Enable a form of non-greedy parsing. Explanation:
                    // http://fastcompression.blogspot.com/2011/12/advanced-parsing-strategies.html
                    for i in 1..PARSE_SEARCH {
                        let mat2 = self.dict.get_match(self.cursor + i);
                        if !mat2.is_empty() {
                            // Check if by skipping 'i' characters we get a
                            // better match. If we do, construct literals and
                            // jump forward.
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
                let mat = mat.start - reduce..mat.end;
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
}

/// An optimal Lempel–Ziv based matcher.
pub struct OptimalMatcher<
    const MAX_OFFSET: usize,
    const MAX_MATCH: usize,
    const DICT_SIZE_BITS: usize,
    const DICT_BANKS: usize,
> {
    matches: Vec<(Range<usize>, Range<usize>)>,
    curr: usize,
}

impl<
        'a,
        const MAX_OFFSET: usize,
        const MAX_MATCH: usize,
        const DICT_SIZE_BITS: usize,
        const DICT_BANKS: usize,
    > OptimalMatcher<MAX_OFFSET, MAX_MATCH, DICT_SIZE_BITS, DICT_BANKS>
{
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            matches: Self::get_matches(input),
            curr: 0,
        }
    }

    fn get_matches(input: &'a [u8]) -> Vec<(Range<usize>, Range<usize>)> {
        let mut dict = LzDictionary::<
            MAX_OFFSET,
            MAX_MATCH,
            DICT_SIZE_BITS,
            DICT_BANKS,
        >::new(input);
        let mut all_matches = Vec::new();
        let input_len = dict.len();

        if input_len <= MIN_MATCH {
            let lit = 0..input_len;
            return vec![(lit, 0..0)];
        }

        // First, collect all of the matches for all of the input.
        for cursor in 0..input_len - MIN_MATCH {
            // Check if there is a previous match, and save the hash.
            let mat = dict.get_match(cursor);
            dict.save_match(cursor);
            all_matches.push(mat);
        }
        for _ in 0..MIN_MATCH {
            all_matches.push(0..0);
        }

        assert_eq!(all_matches.len(), input_len);

        let mut distance_to_end: Vec<usize> = vec![usize::MAX; input_len + 1];

        // Next, figure out which matches are profitable and at what length.
        // This part of the code deletes unprofitable matches and shortens the
        // remaining matches.
        let match_cost = 3;
        distance_to_end[input_len] = 0;
        for i in (0..input_len).rev() {
            let mat = all_matches[i].clone();
            // Cost of not taking the match.
            let no_match_cost = distance_to_end[i + 1] + 1;
            if mat.is_empty() {
                distance_to_end[i] = no_match_cost;
                continue;
            }

            let mut lowest = no_match_cost;
            for len in (MIN_MATCH..mat.len() + 1).rev() {
                let target = i + len;
                // Calculate the cost of the match.
                let taken_match_cost = match_cost + distance_to_end[target];
                // Figure out if it's better to shorten the match.
                if lowest >= taken_match_cost {
                    lowest = taken_match_cost;
                    // Shorten the match.
                    all_matches[i] = mat.start..mat.start + len;
                }
            }
            // Check if it's worthwhile doing a match at all.
            if lowest == no_match_cost || all_matches[i].len() < MIN_MATCH {
                all_matches[i] = 0..0;
            }
            distance_to_end[i] = lowest;
        }

        // Finally, construct the list of selected matches and literal ranges.
        let mut lit = 0..0;
        let mut curr = 0;
        let mut selected_matches = Vec::new();

        while curr < input_len {
            let mat = all_matches[curr].clone();
            if !mat.is_empty() {
                let reduce = dict.grow_match_backwards(&lit, &mat);
                curr += mat.len();
                let lt = lit.start..lit.end - reduce;
                let mt = mat.start - reduce..mat.end;
                selected_matches.push((lt, mt));
                lit = curr..curr;
                continue;
            } else {
                lit = lit.start..lit.end + 1;
                curr += 1;
            }
        }
        selected_matches.push((lit.clone(), 0..0));

        selected_matches
    }
}

/// Implement the iterator trait for the matcher.
impl<
        'a,
        const MAX_OFFSET: usize,
        const MAX_MATCH: usize,
        const DICT_SIZE_BITS: usize,
        const DICT_BANKS: usize,
        const PARSE_SEARCH: usize,
    > Iterator
    for Matcher<
        'a,
        MAX_OFFSET,
        MAX_MATCH,
        DICT_SIZE_BITS,
        DICT_BANKS,
        PARSE_SEARCH,
    >
{
    type Item = (Range<usize>, Range<usize>);

    fn next(&mut self) -> Option<(Range<usize>, Range<usize>)> {
        self.get_next_match_region()
    }
}

/// Implement the iterator trait for the optimal matcher.
impl<
        const MAX_OFFSET: usize,
        const MAX_MATCH: usize,
        const DICT_SIZE_BITS: usize,
        const DICT_BANKS: usize,
    > Iterator
    for OptimalMatcher<MAX_OFFSET, MAX_MATCH, DICT_SIZE_BITS, DICT_BANKS>
{
    type Item = (Range<usize>, Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr < self.matches.len() {
            self.curr += 1;
            return Some(self.matches[self.curr - 1].clone());
        }
        None
    }
}

/// Select the LZ matcher and matcher parameters based on the compression
/// 'level'.
/// 'MAX_LEN' and 'MAX_OFFSET' specify the maximum length and offset of matches.
/// Returns an iterator that iterates over the matches.
pub fn select_matcher<'a, const MAX_OFF: usize, const MAX_LEN: usize>(
    level: u8,
    input: &'a [u8],
) -> Box<dyn Iterator<Item = (Range<usize>, Range<usize>)> + 'a> {
    return match level {
        1 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 16, 2, 1>::new(input)),
        2 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 16, 4, 1>::new(input)),
        3 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 16, 8, 1>::new(input)),
        4 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 16, 8, 2>::new(input)),
        5 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 16, 10, 2>::new(input)),
        6 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 16, 12, 2>::new(input)),
        7 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 17, 12, 2>::new(input)),
        8 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 17, 16, 2>::new(input)),
        9 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 17, 24, 2>::new(input)),
        10 => Box::new(Matcher::<'a, MAX_OFF, MAX_LEN, 20, 128, 4>::new(input)),
        11 => Box::new(OptimalMatcher::<MAX_OFF, MAX_LEN, 21, 128>::new(input)),
        12 => Box::new(OptimalMatcher::<MAX_OFF, MAX_LEN, 22, 256>::new(input)),
        _ => panic!(),
    };
}
