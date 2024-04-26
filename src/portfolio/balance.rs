use crate::utils::serde_utils::f64_default;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

pub type BalanceId = String;

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Balance {
  pub time: DateTime<Utc>,
  pub total: f64,
  pub available: f64,
}

impl Default for Balance {
  fn default() -> Self {
    Self { time: Utc::now(), total: 0.0, available: 0.0 }
  }
}

impl Balance {
  pub fn balance_id(core_id: Uuid) -> BalanceId {
    format!("{}_balance", core_id)
  }
}
