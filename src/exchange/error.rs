use crate::{database::error::DatabaseError, utils::load_config::ConfigError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExchangeError {
  #[error("Binance stream error: {0}")]
  BinanceStreamError(String),
  #[error("Binance client error: {0}")]
  BinanceClientError(String),
  #[error("Exchange didnt fill the order")]
  UnfilledOrder,
  #[error("Failed to serialize/deserialize JSON due to: {0}")]
  JsonSerDe(#[from] serde_json::Error),
  #[error("Init failed {0}")]
  ConfigOnInit(#[from] ConfigError),
}
