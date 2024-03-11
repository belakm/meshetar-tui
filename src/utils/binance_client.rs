use super::load_config::ConfigError;
use crate::utils::load_config::{read_config, ExchangeConfig};
use binance_spot_connector_rust::{http::Credentials, ureq::BinanceHttpClient};
use hyper::client::HttpConnector;
use thiserror::Error;

#[derive(Clone)]
pub struct BinanceClient {
  pub client: BinanceHttpClient,
}

#[derive(Error, Debug)]
pub enum BinanceClientError {
  #[error("Init failed {0}")]
  ConfigOnInit(#[from] ConfigError),
}

impl BinanceClient {
  pub async fn new() -> Result<BinanceClient, BinanceClientError> {
    let config: ExchangeConfig =
      read_config().map_err(|e| BinanceClientError::ConfigOnInit(e))?;
    let credentials =
      Credentials::from_hmac(config.binance_api_key, config.binance_api_secret);
    let client =
      BinanceHttpClient::with_url(&ExchangeConfig::get_exchange_url(config.use_testnet))
        .credentials(credentials);
    Ok(BinanceClient { client })
  }
}
