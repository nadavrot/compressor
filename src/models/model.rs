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
