//! This module contains models that predict the next bit in a sequence of bits.

/// A trait that defines the interface for making predictions.
pub trait Model {
    /// Construct a new model.
    fn new() -> Self;

    /// Return a probability prediction in the 16-bit range using the
    /// internal state.
    #[must_use]
    fn predict(&self) -> u16;

    /// Update the internal context with the next bit 'bit'.
    fn update(&mut self, bit: u8);
}

pub mod bitwise;
pub mod dmc;
pub mod mixer;
