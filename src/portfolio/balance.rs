use crate::utils::serde_utils::f64_default;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, PartialOrd, FromRow)]
pub struct ExchangeBalanceAsset {
    pub id: i64,
    pub asset: String,
    pub free: f64,
    pub locked: f64,
    pub balance_sheet_id: i64,
    #[serde(default = "f64_default")]
    pub btc_valuation: f64,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, PartialOrd)]
pub struct ExchangeBalance {
    pub timestamp: NaiveDateTime,
    pub btc_valuation: f64,
    pub busd_valuation: f64,
    pub balances: Vec<ExchangeBalanceAsset>,
}

#[derive(FromRow, Clone, Serialize)]
pub struct ExchangeBalanceSheet {
    pub id: i64,
    pub timestamp: NaiveDateTime,
    pub btc_valuation: f64,
    pub busd_valuation: f64,
}

pub type BalanceId = String;

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Balance {
    pub time: DateTime<Utc>,
    pub total: f64,
    pub available: f64,
}

impl Default for Balance {
    fn default() -> Self {
        Self {
            time: Utc::now(),
            total: 0.0,
            available: 0.0,
        }
    }
}

impl Balance {
    pub fn balance_id(core_id: Uuid) -> BalanceId {
        format!("{}_balance", core_id)
    }
}
