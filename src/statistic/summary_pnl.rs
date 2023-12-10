use super::{dispersion::Dispersion, welford_online, StatisticConfig, TableBuilder};
use crate::{
    assets::Side,
    portfolio::position::Position,
    utils::serde_utils::{de_duration_from_secs, se_duration_as_secs},
};
use chrono::{DateTime, Duration, Utc};
use prettytable::{row, Row};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default, Deserialize, Serialize)]
pub struct DataSummary {
    pub count: u64,
    pub sum: f64,
    pub mean: f64,
    pub dispersion: Dispersion,
}

impl DataSummary {
    pub fn update(&mut self, next_value: f64) {
        // Increment counter
        self.count += 1;

        // Update Sum
        self.sum += next_value;

        // Update Mean
        let prev_mean = self.mean;
        self.mean = welford_online::calculate_mean(self.mean, next_value, self.count as f64);

        // Update Dispersion
        self.dispersion
            .update(prev_mean, self.mean, next_value, self.count);
    }
}

impl TableBuilder for DataSummary {
    fn titles(&self) -> Row {
        row![
            "Count",
            "Sum",
            "Mean",
            "Variance",
            "Std. Dev",
            "Range High",
            "Range Low",
        ]
    }

    fn row(&self) -> Row {
        row![
            self.count,
            format!("{:.8}", self.sum),
            format!("{:.8}", self.mean),
            format!("{:.8}", self.dispersion.variance),
            format!("{:.8}", self.dispersion.std_dev),
            format!("{:.8}", self.dispersion.range.high),
            format!("{:.8}", self.dispersion.range.low),
        ]
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct PnLReturnSummary {
    pub time: DateTime<Utc>,
    #[serde(
        deserialize_with = "de_duration_from_secs",
        serialize_with = "se_duration_as_secs"
    )]
    pub duration: Duration,
    pub trades_per_day: f64,
    pub total: DataSummary,
    pub losses: DataSummary,
}

impl TableBuilder for PnLReturnSummary {
    fn titles(&self) -> Row {
        row![
            "Trades",
            "Wins",
            "Losses",
            "Trading Days",
            "Trades Per Day",
            "Mean Return",
            "Std. Dev. Return",
            "Loss Mean Return",
            "Biggest Win",
            "Biggest Loss",
        ]
    }

    fn row(&self) -> Row {
        let wins = self.total.count - self.losses.count;
        row![
            self.total.count.to_string(),
            wins,
            self.losses.count,
            self.duration.num_days().to_string(),
            format!("{:.8}", self.trades_per_day),
            format!("{:.8}", self.total.mean),
            format!("{:.8}", self.total.dispersion.std_dev),
            format!("{:.8}", self.losses.mean),
            format!("{:.8}", self.total.dispersion.range.high),
            format!("{:.8}", self.total.dispersion.range.low),
        ]
    }
}

impl Default for PnLReturnSummary {
    fn default() -> Self {
        Self {
            time: Utc::now(),
            duration: Duration::zero(),
            trades_per_day: 0.0,
            total: DataSummary::default(),
            losses: DataSummary::default(),
        }
    }
}

impl PnLReturnSummary {
    const SECONDS_IN_DAY: f64 = 86400.0;

    pub fn new(starting_time: DateTime<Utc>) -> Self {
        Self {
            time: starting_time,
            duration: Duration::zero(),
            trades_per_day: 0.0,
            total: Default::default(),
            losses: Default::default(),
        }
    }

    pub fn update_trading_session_duration(&mut self, position: &Position) {
        self.duration = match position.meta.exit_balance {
            None => {
                // Since Position is not exited, estimate duration w/ last_update_time
                position.meta.update_time.signed_duration_since(self.time)
            }
            Some(exit_balance) => exit_balance.time.signed_duration_since(self.time),
        }
    }

    pub fn update_trades_per_day(&mut self) {
        self.trades_per_day = self.total.count as f64
            / (self.duration.num_seconds() as f64 / PnLReturnSummary::SECONDS_IN_DAY)
    }

    pub fn init(_: StatisticConfig) -> Self {
        Self::default()
    }

    pub fn update(&mut self, position: &Position) {
        // Set start timestamp if it's the first trade of the session
        if self.total.count == 0 {
            self.time = position.meta.enter_time;
        }

        // Update duration of trading session & trades per day
        self.update_trading_session_duration(position);
        self.update_trades_per_day();

        // Calculate the Position PnL Return
        let pnl_return = position.calculate_profit_loss_return();

        // Update Total PnL Returns
        self.total.update(pnl_return);

        // Update Loss PnL Returns if relevant
        if pnl_return.is_sign_negative() {
            self.losses.update(pnl_return);
        }
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default, Deserialize, Serialize)]
pub struct ProfitLossSummary {
    pub long_contracts: f64,
    pub long_pnl: f64,
    pub long_pnl_per_contract: f64,
    pub short_contracts: f64,
    pub short_pnl: f64,
    pub short_pnl_per_contract: f64,
    pub total_contracts: f64,
    pub total_pnl: f64,
    pub total_pnl_per_contract: f64,
}

impl ProfitLossSummary {
    pub fn update(&mut self, position: &Position) {
        self.total_contracts += position.quantity.abs();
        self.total_pnl += position.realised_profit_loss;
        self.total_pnl_per_contract = self.total_pnl / self.total_contracts;

        match position.side {
            Side::Buy => {
                self.long_contracts += position.quantity.abs();
                self.long_pnl += position.realised_profit_loss;
                self.long_pnl_per_contract = self.long_pnl / self.long_contracts;
            }
            Side::Sell => {
                self.short_contracts += position.quantity.abs();
                self.short_pnl += position.realised_profit_loss;
                self.short_pnl_per_contract = self.short_pnl / self.short_contracts;
            }
        }
    }

    pub fn new() -> Self {
        Self::default()
    }
}

impl TableBuilder for ProfitLossSummary {
    fn titles(&self) -> Row {
        row![
            "Long Contracts",
            "Long PnL",
            "Long PnL Per Contract",
            // "Short Contracts",
            // "Short PnL",
            // "Short PnL Per Contract",
            // "Total Contracts",
            // "Total PnL",
            // "Total PnL Per Contract",
        ]
    }

    fn row(&self) -> Row {
        row![
            format!("{:.8}", self.long_contracts),
            format!("{:.8}", self.long_pnl),
            format!("{:.8}", self.long_pnl_per_contract),
            // format!("{:.3}", self.short_contracts),
            // format!("{:.3}", self.short_pnl),
            // format!("{:.3}", self.short_pnl_per_contract),
            // format!("{:.3}", self.total_contracts),
            // format!("{:.3}", self.total_pnl),
            // format!("{:.3}", self.total_pnl_per_contract),
        ]
    }
}
