use crate::{
    statistic::{dispersion::Range, welford_online},
    utils::serde_utils::{de_duration_from_secs, se_duration_as_secs},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::EquityPoint;

/// [`Drawdown`] is the peak-to-trough decline of the Portfolio, or investment, during a specific
/// period. Drawdown is a measure of downside volatility.
///
/// See documentation: <https://www.investopedia.com/terms/d/drawdown.asp>
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Drawdown {
    pub equity_range: Range,
    pub drawdown: f64,
    pub start_time: DateTime<Utc>,
    #[serde(
        deserialize_with = "de_duration_from_secs",
        serialize_with = "se_duration_as_secs"
    )]
    pub duration: Duration,
}

impl Default for Drawdown {
    fn default() -> Self {
        Self {
            equity_range: Default::default(),
            drawdown: 0.0,
            start_time: Utc::now(),
            duration: Duration::zero(),
        }
    }
}

impl Drawdown {
    /// Initialises a new [`Drawdown`] using the starting equity as the first peak.
    pub fn init(starting_equity: f64) -> Self {
        Self {
            equity_range: Range {
                activated: true,
                high: starting_equity,
                low: starting_equity,
            },
            drawdown: 0.0,
            start_time: Utc::now(),
            duration: Duration::zero(),
        }
    }

    /// Updates the [`Drawdown`] using the latest input [`EquityPoint`] of the Portfolio. If the drawdown
    /// period has ended (investment recovers from a trough back above the previous peak), the
    /// function return Some(Drawdown), else None is returned.
    pub fn update(&mut self, current: EquityPoint) -> Option<Drawdown> {
        match (
            self.is_waiting_for_peak(),
            current.total > self.equity_range.high,
        ) {
            // A) No current drawdown - waiting for next equity peak (waiting for B)
            (true, true) => {
                self.equity_range.high = current.total;
                None
            }

            // B) Start of new drawdown - previous equity point set peak & current equity lower
            (true, false) => {
                self.start_time = current.time;
                self.equity_range.low = current.total;
                self.drawdown = self.calculate();
                None
            }

            // C) Continuation of drawdown - equity lower than most recent peak
            (false, false) => {
                self.duration = current.time.signed_duration_since(self.start_time);
                self.equity_range.update(current.total);
                self.drawdown = self.calculate(); // I don't need to calculate this now if I don't want
                None
            }

            // D) End of drawdown - equity has reached new peak (enters A)
            (false, true) => {
                // Clone Drawdown from previous iteration to return
                let finished_drawdown = Drawdown {
                    equity_range: self.equity_range,
                    drawdown: self.drawdown,
                    start_time: self.start_time,
                    duration: self.duration,
                };

                // Clean up - start_time overwritten next drawdown start
                self.drawdown = 0.0; // ie/ waiting for peak = true
                self.duration = Duration::zero();

                // Set new equity peak in preparation for next iteration
                self.equity_range.high = current.total;

                Some(finished_drawdown)
            }
        }
    }

    /// Determines if a [`Drawdown`] is waiting for the next equity peak. This is true if the new
    /// [`EquityPoint`] is higher than the previous peak.
    pub fn is_waiting_for_peak(&self) -> bool {
        self.drawdown == 0.0
    }

    /// Calculates the value of the [`Drawdown`] in the specific period. Uses the formula:
    /// [`Drawdown`] = (range_low - range_high) / range_high
    pub fn calculate(&self) -> f64 {
        // range_low - range_high / range_high
        (-self.equity_range.calculate()) / self.equity_range.high
    }
}

/// [`MaxDrawdown`] is the largest
/// peak-to-trough decline of the Portfolio, or investment. Max Drawdown is a measure of downside
/// risk, with large values indicating down movements could be volatile.
///
/// See documentation: <https://www.investopedia.com/terms/m/maximum-drawdown-mdd.asp>
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct MaxDrawdown {
    pub drawdown: Drawdown,
}

impl MaxDrawdown {
    /// Initialises a new [`MaxDrawdown`] using the [`Drawdown`] default value.
    pub fn init() -> Self {
        Self {
            drawdown: Drawdown::default(),
        }
    }

    /// Updates the [`MaxDrawdown`] using the latest input [`Drawdown`] of the Portfolio. If the input
    /// drawdown is larger than the current [`MaxDrawdown`], it supersedes it.
    pub fn update(&mut self, next_drawdown: &Drawdown) {
        if next_drawdown.drawdown.abs() > self.drawdown.drawdown.abs() {
            self.drawdown = *next_drawdown;
        }
    }
}

/// [`AvgDrawdown`] contains the average drawdown value and duration from a collection of [`Drawdown`]s
/// within a specific period.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct AvgDrawdown {
    pub count: u64,
    pub mean_drawdown: f64,
    #[serde(
        deserialize_with = "de_duration_from_secs",
        serialize_with = "se_duration_as_secs"
    )]
    pub mean_duration: Duration,
    pub mean_duration_milliseconds: i64,
}

impl Default for AvgDrawdown {
    fn default() -> Self {
        Self {
            count: 0,
            mean_drawdown: 0.0,
            mean_duration_milliseconds: 0,
            mean_duration: Duration::zero(),
        }
    }
}

impl AvgDrawdown {
    /// Initialises a new [`AvgDrawdown`] using the default method, providing zero values for all
    /// fields.
    pub fn init() -> Self {
        Self::default()
    }

    /// Updates the [`AvgDrawdown`] using the latest input [`Drawdown`] of the Portfolio.
    pub fn update(&mut self, drawdown: &Drawdown) {
        self.count += 1;

        self.mean_drawdown = welford_online::calculate_mean(
            self.mean_drawdown,
            drawdown.drawdown,
            self.count as f64,
        );

        self.mean_duration_milliseconds = welford_online::calculate_mean(
            self.mean_duration_milliseconds,
            drawdown.duration.num_milliseconds(),
            self.count as i64,
        );

        self.mean_duration = Duration::milliseconds(self.mean_duration_milliseconds);
    }
}
