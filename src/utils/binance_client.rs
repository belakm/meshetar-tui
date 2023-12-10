use crate::utils::load_config::{read_config, Config};
use binance_spot_connector_rust::{http::Credentials, hyper::BinanceHttpClient};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use thiserror::Error;

use super::load_config::ConfigError;

pub const BINANCE_WSS_BASE_URL: &str = "wss://stream.binance.com:9443/ws";

#[derive(Clone)]
pub struct BinanceClient {
    pub client: BinanceHttpClient<HttpsConnector<HttpConnector>>,
}

#[derive(Error, Debug)]
pub enum BinanceClientError {
    #[error("Init failed {0}")]
    ConfigOnInit(#[from] ConfigError),
}

impl BinanceClient {
    pub async fn new() -> Result<BinanceClient, BinanceClientError> {
        let config: Config = read_config().map_err(|e| BinanceClientError::ConfigOnInit(e))?;
        let credentials = Credentials::from_hmac(
            config.binance_api_key.to_owned(),
            config.binance_api_secret.to_owned(),
        );
        let client =
            // Testnet: 
            // BinanceHttpClient::with_url("https://testnet.binance.vision").credentials(credentials);
            // Realnet: 
            BinanceHttpClient::with_url("https://api.binance.com").credentials(credentials);
        Ok(BinanceClient { client })
    }
}
