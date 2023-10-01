//! This module contains the implementation of a model mixer.

use super::bitwise::{BitwiseModel, MODEL_CTX, MODEL_LIMIT};
use super::dmc::DMCModel;
use super::Model;

type BitwiseModelType = BitwiseModel<MODEL_CTX, MODEL_LIMIT>;

/// A Model that mixes the two other models that are implemented in this module.
/// The prediction is done by averaging the two predictions, that have an equal
/// weight.
pub struct Mixer {
    model0: DMCModel,
    model1: BitwiseModelType,
}

impl Model for Mixer {
    fn new() -> Self {
        let model0 = DMCModel::new();
        let model1 = BitwiseModelType::new();
        Mixer { model0, model1 }
    }

    fn predict(&self) -> u16 {
        let p0 = self.model0.predict();
        let p1 = self.model1.predict();
        p0 / 2 + p1 / 2
    }

    fn update(&mut self, bit: u8) {
        self.model0.update(bit);
        self.model1.update(bit);
    }
}
