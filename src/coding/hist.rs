//! A Histogram that can normalize values and display them.

pub struct Histogram<const BINS: usize> {
    values: [u32; BINS],
}

impl<const BINS: usize> Histogram<BINS> {
    pub fn from_data<Ty: Into<usize> + Copy>(values: &[Ty]) -> Histogram<BINS> {
        // Fill the histogram in four buckets to allow instruction-level
        // parallelism.
        let mut hist0 = [0; BINS];
        let mut hist1 = [0; BINS];
        let mut hist2 = [0; BINS];
        let mut hist3 = [0; BINS];

        let mut i = 0;
        while i + 3 < values.len() {
            hist0[Into::into(values[i])] += 1;
            hist1[Into::into(values[i + 1])] += 1;
            hist2[Into::into(values[i + 2])] += 1;
            hist3[Into::into(values[i + 3])] += 1;
            i += 4;
        }
        while i < values.len() {
            hist0[Into::into(values[i])] += 1;
            i += 1;
        }

        let mut hist = [0; BINS];
        for i in 0..BINS {
            hist[i] = hist0[i] + hist1[i] + hist2[i] + hist3[i];
        }

        Histogram { values: hist }
    }

    pub fn get_bins(&self) -> &[u32; BINS] {
        &self.values
    }

    pub fn dump(&self) {
        let mut first_non_zero = BINS;
        let mut last_non_zero = 0;
        let mut max = 0;

        // Find the max value and the non-zero ranges.
        for (i, val) in self.values.iter().enumerate() {
            if *val != 0 {
                first_non_zero = first_non_zero.min(i);
                last_non_zero = last_non_zero.max(i);
                max = max.max(*val);
            }
        }

        if max == 0 {
            println!("-- empty --");
        }

        fn print_bar(index: usize, value: usize, len: usize) {
            print!("{}) ", index);
            for _ in 0..len {
                print!("#");
            }
            println!(" - {}", value);
        }

        // Print the values.
        for i in first_non_zero..last_non_zero + 1 {
            let dots = 40 * self.values[i] / max;
            print_bar(i, self.values[i] as usize, dots as usize);
        }
    }
}

/// Returns the number of bits needed to represent the word 'num'.
pub fn num_bits(num: u32) -> u32 {
    32 - num.leading_zeros()
}

/// Normalize 'values' to make the sum of the values equal to 'total'.
/// This keeps non-zero values as non zero.
/// The value of 'total' must be greater than the number of bins in 'values'.
/// Reference: FSE_normalizeCount and FSE_normalizeM2
pub fn normalize_to_total_sum(values: &mut [u32], total: u32) {
    let mut new_values: Vec<u32> = values.to_vec();
    assert!(total > values.len() as u32);

    // Make sure to reserve some of the budget for bumping all of the non zeros
    // up by one.
    let non_zeros = values.iter().filter(|&n| *n != 0).count() as u32;

    let sum: u32 = values.iter().sum();

    // Spread all of the values as well as possible within the total-non_zero
    // budget.
    let mut max_value_idx = 0;
    if sum > 0 {
        for i in 0..values.len() {
            new_values[i] = ((values[i] as u64 * (total - non_zeros) as u64)
                / sum as u64) as u32;

            // Find the highest value.
            if values[i] > values[max_value_idx] {
                max_value_idx = i;
            }
        }
    }

    let sum = new_values.iter().sum();
    assert!(
        total >= sum,
        "Expected the sum to be lower because we round down"
    );
    let mut gap = total - sum;

    // Turn zeros into 1s. This is needed for correctness.
    for i in 0..values.len() {
        if new_values[i] == 0 && values[i] != 0 {
            new_values[i] += 1;
            gap -= 1;
        }
        if gap == 0 {
            break;
        }
    }

    // Write the remaining values to the highest value.
    new_values[max_value_idx] += gap;

    values.copy_from_slice(&new_values);

    let sum: u32 = values.iter().sum();
    assert!(sum == total);
}

impl<const BINS: usize> Histogram<BINS> {
    pub fn normalize(&mut self, total_sum: usize) {
        normalize_to_total_sum(&mut self.values, total_sum as u32);
    }
}
