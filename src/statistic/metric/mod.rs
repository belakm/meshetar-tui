use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::portfolio::{balance::Balance, position::Position};

pub mod drawdown;
pub mod ratio;

/// Total equity at a point in time - equates to [`Balance.total`](Balance).
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct EquityPoint {
    pub time: DateTime<Utc>,
    pub total: f64,
}

impl Default for EquityPoint {
    fn default() -> Self {
        Self {
            time: Utc::now(),
            total: 0.0,
        }
    }
}

impl From<Balance> for EquityPoint {
    fn from(balance: Balance) -> Self {
        Self {
            time: balance.time,
            total: balance.total,
        }
    }
}

impl EquityPoint {
    /// Updates using the input [`Position`]'s PnL & associated timestamp.
    fn update(&mut self, position: &Position) {
        match position.meta.exit_balance {
            None => {
                // Position is not exited, so simulate
                self.time = position.meta.update_time;
                self.total += position.unrealised_profit_loss;
            }
            Some(exit_balance) => {
                self.time = exit_balance.time;
                self.total += position.realised_profit_loss;
            }
        }
    }
}
