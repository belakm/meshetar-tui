use crate::utils::load_config::{read_config, Config};
use binance_spot_connector_rust::{http::Credentials, ureq::BinanceHttpClient};
use hyper::client::HttpConnector;
use thiserror::Error;

use super::load_config::ConfigError;

// TODO: Read this from .config/env.toml
pub const BINANCE_WSS_BASE_URL: &str = "wss://testnet.binance.vision/ws";

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
    let config: Config =
      read_config().map_err(|e| BinanceClientError::ConfigOnInit(e))?;
    let credentials =
      Credentials::from_hmac(config.binance_api_key, config.binance_api_secret);
    let client =
      BinanceHttpClient::with_url(&config.binance_url).credentials(credentials);
    Ok(BinanceClient { client })
  }
}
