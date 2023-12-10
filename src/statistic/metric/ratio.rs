use crate::statistic::summary_pnl::PnLReturnSummary;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct SharpeRatio {
    pub risk_free_return: f64,
    pub trades_per_day: f64,
    pub sharpe_ratio_per_trade: f64,
}

impl SharpeRatio {
    pub fn update(&mut self, pnl_returns: &PnLReturnSummary) {
        // Update Trades Per Day
        self.trades_per_day = pnl_returns.trades_per_day;

        // Calculate Sharpe Ratio Per Trade
        self.sharpe_ratio_per_trade = match pnl_returns.total.dispersion.std_dev == 0.0 {
            true => 0.0,
            false => {
                (pnl_returns.total.mean - self.risk_free_return)
                    / pnl_returns.total.dispersion.std_dev
            }
        };
    }
    pub fn init(risk_free_return: f64) -> Self {
        Self {
            risk_free_return,
            sharpe_ratio_per_trade: 0.0,
            trades_per_day: 0.0,
        }
    }

    pub fn ratio(&self) -> f64 {
        self.sharpe_ratio_per_trade
    }

    pub fn trades_per_day(&self) -> f64 {
        self.trades_per_day
    }

    pub fn daily(&self) -> f64 {
        calculate_daily(self.ratio(), self.trades_per_day())
    }
    pub fn annual(&self, trading_days: u32) -> f64 {
        calculate_annual(self.ratio(), self.trades_per_day(), trading_days)
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct SortinoRatio {
    pub risk_free_return: f64,
    pub trades_per_day: f64,
    pub sortino_ratio_per_trade: f64,
}

impl SortinoRatio {
    pub fn init(risk_free_return: f64) -> Self {
        Self {
            risk_free_return,
            trades_per_day: 0.0,
            sortino_ratio_per_trade: 0.0,
        }
    }

    pub fn ratio(&self) -> f64 {
        self.sortino_ratio_per_trade
    }

    pub fn trades_per_day(&self) -> f64 {
        self.trades_per_day
    }

    pub fn update(&mut self, pnl_returns: &PnLReturnSummary) {
        // Update Trades Per Day
        self.trades_per_day = pnl_returns.trades_per_day;

        // Calculate Sortino Ratio Per Trade
        self.sortino_ratio_per_trade = match pnl_returns.losses.dispersion.std_dev == 0.0 {
            true => 0.0,
            false => {
                (pnl_returns.total.mean - self.risk_free_return)
                    / pnl_returns.losses.dispersion.std_dev
            }
        };
    }

    pub fn daily(&self) -> f64 {
        calculate_daily(self.ratio(), self.trades_per_day())
    }
    pub fn annual(&self, trading_days: u32) -> f64 {
        calculate_annual(self.ratio(), self.trades_per_day(), trading_days)
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct CalmarRatio {
    pub risk_free_return: f64,
    pub trades_per_day: f64,
    pub calmar_ratio_per_trade: f64,
}

impl CalmarRatio {
    pub fn init(risk_free_return: f64) -> Self {
        Self {
            risk_free_return,
            trades_per_day: 0.0,
            calmar_ratio_per_trade: 0.0,
        }
    }

    pub fn ratio(&self) -> f64 {
        self.calmar_ratio_per_trade
    }

    pub fn trades_per_day(&self) -> f64 {
        self.trades_per_day
    }

    pub fn update(&mut self, pnl_returns: &PnLReturnSummary, max_drawdown: f64) {
        // Update Trades Per Day
        self.trades_per_day = pnl_returns.trades_per_day;

        // Calculate Calmar Ratio Per Trade
        self.calmar_ratio_per_trade = match max_drawdown == 0.0 {
            true => 0.0,
            false => (pnl_returns.total.mean - self.risk_free_return) / max_drawdown.abs(),
        };
    }

    pub fn daily(&self) -> f64 {
        calculate_daily(self.ratio(), self.trades_per_day())
    }
    pub fn annual(&self, trading_days: u32) -> f64 {
        calculate_annual(self.ratio(), self.trades_per_day(), trading_days)
    }
}

pub fn calculate_daily(ratio_per_trade: f64, trades_per_day: f64) -> f64 {
    ratio_per_trade * trades_per_day.sqrt()
}

pub fn calculate_annual(ratio_per_trade: f64, trades_per_day: f64, trading_days: u32) -> f64 {
    calculate_daily(ratio_per_trade, trades_per_day) * (trading_days as f64).sqrt()
}
