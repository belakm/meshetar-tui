use crate::statistic::welford_online;
use serde::{Deserialize, Serialize};

/// Representation of a dataset using measures of dispersion - range, variance & standard deviation.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default, Deserialize, Serialize)]
pub struct Dispersion {
    pub range: Range,
    pub recurrence_relation_m: f64,
    pub variance: f64,
    pub std_dev: f64,
}

impl Dispersion {
    /// Iteratively updates the measures of Dispersion given the previous mean, new mean, new value,
    /// and the dataset count.
    pub fn update(&mut self, prev_mean: f64, new_mean: f64, new_value: f64, value_count: u64) {
        // Update Range
        self.range.update(new_value);

        // Update Welford Online recurrence relation M
        self.recurrence_relation_m = welford_online::calculate_recurrence_relation_m(
            self.recurrence_relation_m,
            prev_mean,
            new_value,
            new_mean,
        );

        // Update Population Variance
        self.variance =
            welford_online::calculate_population_variance(self.recurrence_relation_m, value_count);

        // Update Standard Deviation
        self.std_dev = self.variance.sqrt();
    }
}

/// Measure of dispersion providing the highest and lowest value of a dataset. Lazy evaluation is
/// used when calculating the range between them via the calculate() function.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default, Deserialize, Serialize)]
pub struct Range {
    pub activated: bool,
    pub high: f64,
    pub low: f64,
}

impl Range {
    /// Initialises the Range with the provided first value of the dataset.
    pub fn init(first_value: f64) -> Self {
        Self {
            activated: false,
            high: first_value,
            low: first_value,
        }
    }

    /// Iteratively updates the Range given the next value in the dataset.
    pub fn update(&mut self, new_value: f64) {
        match self.activated {
            true => {
                if new_value > self.high {
                    self.high = new_value;
                }

                if new_value < self.low {
                    self.low = new_value;
                }
            }
            false => {
                self.activated = true;
                self.high = new_value;
                self.low = new_value;
            }
        }
    }

    /// Calculates the range between the highest and lowest value of a dataset. Provided to
    /// allow lazy evaluation.
    pub fn calculate(&self) -> f64 {
        self.high - self.low
    }
}
