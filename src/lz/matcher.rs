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

    /// Return a match to a previous string that's equal to the string that
    /// starts at 'idx'. The value 'prev_best' is the length of some previous
    /// match. Don't bother finding matches that are shorter than this value.
    /// 'cache_key' is the index to the bank way to search.
    fn get_match(
        &self,
        idx: usize,
        mut prev_best: usize,
        cache_key: usize,
    ) -> Range<usize> {
        debug_assert_eq!(cache_key, self.get_match_candidate(idx));
        let mut best = 0..0;

        for i in 0..DICT_BANKS {
            let loc = self.dict[cache_key * DICT_BANKS + i];
            // Ignore empty cells.
            if loc == EMPTY_CELL {
                break;
            }
            // Ignore match distances that are too big.
            let offset = idx - loc as usize;
            if offset >= MAX_OFFSET {
                break;
            }
            if self.early_disqualify(loc as usize, idx, prev_best) {
                continue;
            }
            let len = self.get_match_length(loc as usize, idx);
            if best.len() < len {
                best = (loc as usize)..(loc as usize) + len;
                prev_best = prev_best.max(len);
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
    /// Save the value at index 'idx' to cache entry at 'cache_key' and rotate
    /// the entries in the cache.
    fn save_match(&mut self, idx: usize, cache_key: usize) {
        debug_assert_eq!(cache_key, self.get_match_candidate(idx));

        // This is an LRU cache. Move the old entries to make room to the new
        // entry.
        let base = cache_key * DICT_BANKS;
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
        lit: &mut Range<usize>,
        mat: &mut Range<usize>,
    ) -> usize {
        // We go lit_len steps backwards, so don't overflow. Also, don't handle
        // empty match or literal packets.
        if mat.start <= lit.len() || (*mat).is_empty() || (*lit).is_empty() {
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
        *lit = lit.start..(lit.end - i);
        *mat = (mat.start - i)..mat.end;
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

        // Candidate: lit, match, index (match start point), cursor after match.
        let mut candidate: Option<(Range<usize>, Range<usize>, usize, usize)> =
            None;

        // For each character in the input buffer:
        while self.cursor + MIN_MATCH < input_len {
            // If we've exceeded the window, return the candidate.
            if let Some(can) = &candidate {
                if self.cursor >= can.2 + PARSE_SEARCH {
                    debug_assert!(self.cursor <= can.3);
                    // When accepting a match, hash the content of the match.
                    for i in self.cursor..(can.3).min(input_len - MIN_MATCH) {
                        let cache_key = self.dict.get_match_candidate(i);
                        self.dict.save_match(i, cache_key);
                    }
                    self.cursor = can.3;
                    let mut lit = can.0.clone();
                    let mut mat = can.1.clone();
                    self.dict.grow_match_backwards(&mut lit, &mut mat);
                    debug_assert!(mat.end < input_len);
                    return Some((lit, mat));
                }
            }

            let mut prev = 0;
            if let Some(can) = &candidate {
                prev = can.1.len();
            }

            // Check if there is a previous match and save the hash.
            let cache_key = self.dict.get_match_candidate(self.cursor);
            let mat = self.dict.get_match(self.cursor, prev, cache_key);
            self.dict.save_match(self.cursor, cache_key);

            if mat.is_empty() {
                // We didn't find a match. Grow the literal region and move on.
                self.cursor += 1;
                lit = lit.start..lit.end + 1;
                continue;
            }

            // If we don't have a previous candidate, save this match as a
            // candidate.
            if candidate.is_none() {
                candidate = Some((
                    lit.clone(),
                    mat.clone(),
                    self.cursor,
                    self.cursor + mat.len(),
                ));

                // And continue to the next character.
                self.cursor += 1;
                lit = lit.start..lit.end + 1;
                continue;
            }

            // If we have a new match and a previous candidate, select between
            // them.
            if let Some(can) = &candidate {
                // Distance between where the candidate started and where the
                // new match starts will be filled with literals.
                let lit_len = self.cursor - can.2;

                let candidate_size = can.1.len();
                let new_match_size = mat.len();
                let new_mat_closer = can.1.start < mat.start;
                // Check if the new match is bigger. We include the size of the
                // extra literals in this calculation. If the size is the same
                // then break the tie by looking at the offset of the match,
                // where lower is better.
                let better_size = new_match_size > candidate_size + lit_len;
                let same_size_better_offset = new_match_size
                    == candidate_size + lit_len
                    && new_mat_closer;
                if better_size || same_size_better_offset {
                    // Pick a new match candidate.
                    debug_assert!(can.3 < self.cursor + mat.len());
                    candidate = Some((
                        lit.clone(),
                        mat.clone(),
                        self.cursor,
                        self.cursor + mat.len(),
                    ));

                    self.cursor += 1;
                    lit = lit.start..lit.end + 1;
                    continue;
                } else {
                    // Stay with the current match candidate.
                    self.cursor += 1;
                    lit = lit.start..lit.end + 1;
                    continue;
                }
            }
        } // End of main loop.

        // We finished scanning the buffer. Return the last candidate.
        if let Some(can) = &candidate {
            debug_assert!(self.cursor < can.3);
            self.cursor = can.3;
            let mut lit = can.0.clone();
            let mut mat = can.1.clone();
            self.dict.grow_match_backwards(&mut lit, &mut mat);

            debug_assert!(mat.end < input_len);
            return Some((lit, mat));
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
            let cache_key = dict.get_match_candidate(cursor);
            let mat = dict.get_match(cursor, 0, cache_key);
            dict.save_match(cursor, cache_key);
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
            let mut mat = all_matches[curr].clone();
            if !mat.is_empty() {
                dict.grow_match_backwards(&mut lit, &mut mat);
                curr += mat.len();
                selected_matches.push((lit, mat));
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
