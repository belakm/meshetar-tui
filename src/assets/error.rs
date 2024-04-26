use thiserror::Error;

use crate::{database::error::DatabaseError, exchange::error::ExchangeError};

#[derive(Error, Debug)]
pub enum AssetError {
  #[error("Failed to serialize/deserialize JSON due to: {0}")]
  JsonSerDe(#[from] serde_json::Error),
  #[error("Database error: {0}")]
  DatabaseError(#[from] DatabaseError),
  #[error("Exchange error: {0}")]
  ExchangeError(#[from] ExchangeError),
}
