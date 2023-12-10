use thiserror::Error;

use crate::database::error::DatabaseError;

#[derive(Error, Debug)]
pub enum AssetError {
    #[error("Binance stream error: {0}")]
    BinanceStreamError(String),
    #[error("Binance client error: {0}")]
    BinanceClientError(String),
    #[error("Failed to serialize/deserialize JSON due to: {0}")]
    JsonSerDe(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Amount of candles to skip should be 0 or more.")]
    NegativeCandleNumberSkip,
}
