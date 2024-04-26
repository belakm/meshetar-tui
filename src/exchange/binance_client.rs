use super::error::ExchangeError;
use crate::utils::load_config::{read_config, ConfigError, ExchangeConfig};
use binance_spot_connector_rust::{http::Credentials, ureq::BinanceHttpClient};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone)]
pub struct BinanceClient {
  pub client: BinanceHttpClient,
}

#[derive(Error, Debug)]
pub enum BinanceClientError {
  #[error("Init failed {0}")]
  ConfigOnInit(#[from] ConfigError),
  #[error("Stream key error: {0}")]
  KeyError(String),
  #[error("Failed to serialize/deserialize JSON due to: {0}")]
  JsonSerDe(#[from] serde_json::Error),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceRawKey {
  pub listen_key: String,
}

impl BinanceClient {
  pub async fn new() -> Result<BinanceClient, ExchangeError> {
    let config: ExchangeConfig =
      read_config().map_err(|e| ExchangeError::ConfigOnInit(e))?;

    let credentials =
      Credentials::from_hmac(config.binance_api_key, config.binance_api_secret);

    let client =
      BinanceHttpClient::with_url(&ExchangeConfig::get_exchange_url(config.use_testnet))
        .credentials(credentials);
    Ok(BinanceClient { client })
  }

  pub async fn credentials() -> Result<Credentials, ExchangeError> {
    let config: ExchangeConfig =
      read_config().map_err(|e| ExchangeError::ConfigOnInit(e))?;

    let credentials =
      Credentials::from_hmac(config.binance_api_key, config.binance_api_secret);

    Ok(credentials)
  }

  pub async fn get_stream_key(&self) -> Result<String, ExchangeError> {
    let key = self
      .client
      .send(binance_spot_connector_rust::stream::new_listen_key())
      .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;
    let key = key
      .into_body_str()
      .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;

    let key: BinanceRawKey = serde_json::from_str(&key)?;
    Ok(key.listen_key)
  }
}
