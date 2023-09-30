use crate::utils::RECIPROCAL_U32;

use super::model::Model;

pub const MODEL_CTX: usize = 29;
pub const MODEL_LIMIT: usize = 400;

/// A simple model that predicts the probability of the next bit.
/// CONTEXT_SIZE_BITS defines the size of the cache (history).
/// LIMIT defines the maximum number of samples for bucket.
pub struct BitwiseModel<const CONTEXT_SIZE_BITS: usize, const LIMIT: usize> {
    ctx: u64,
    cache: Vec<(u16, u16)>,
}

impl<const CTX_SIZE_BITS: usize, const LIMIT: usize> Model
    for BitwiseModel<CTX_SIZE_BITS, LIMIT>
{
    fn new() -> Self {
        Self {
            ctx: 0,
            cache: vec![(1, 1); 1 << CTX_SIZE_BITS],
        }
    }

    fn predict(&self) -> u16 {
        // Return a probability prediction in the 16-bit range using the
        // 'CTX_SIZE_BITS' LSB bits in 'ctx'.
        let key = self.ctx % (1 << CTX_SIZE_BITS);
        let (set, cnt) = self.cache[key as usize];
        debug_assert!(cnt < 1024);
        let a = set as u64;
        let b = 1 + cnt as u64;

        // This is equivalent to (a * (1<<16)) / b;
        ((a * (RECIPROCAL_U32[b as usize] as u64)) >> 16) as u16
    }

    fn update(&mut self, bit: u8) {
        // Update the probability of the context 'ctx', considering the first
        // 'CTX_SIZE_BITS' LSB bits, with the bit 'bit'.
        let key = self.ctx % (1 << CTX_SIZE_BITS);
        let (set, cnt) = &mut self.cache[key as usize];
        *cnt += 1;
        *set += (bit & 1) as u16;
        // Normalize the count if LIMIT is exceeded. This allows new data to
        // have a higher weight.
        if *cnt as usize >= LIMIT {
            *set /= 2;
            *cnt /= 2;
        }
        // Update the context.
        self.ctx = (self.ctx << 1) + bit as u64;
    }
}

#[test]
fn test_simple_model() {
    {
        let mut model = BitwiseModel::<7, 1024>::new();
        for _ in 0..10000 {
            model.update(1);
            model.update(0);
        }

        // Predict a '1'
        let pred = model.predict();
        assert!(pred > 64_000);
        model.update(1);

        // Predict a zero.
        let pred = model.predict();
        assert!(pred < 1_000);
    }

    {
        let mut model = BitwiseModel::<7, 256>::new();
        for _ in 0..10000 {
            model.update(0);
        }
        // The prediction needs to be close to zero.
        let pred = model.predict();
        assert_eq!(pred, 0);
    }

    {
        let mut model = BitwiseModel::<7, 256>::new();
        for _ in 0..10000 {
            model.update(1);
        }
        // The prediction needs to be close to one.
        let pred = model.predict();
        assert!(pred > 65_000);
    }
}

