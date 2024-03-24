use thiserror::Error;

#[derive(serde::Deserialize, Debug)]
pub struct UserConfig {
  binance_api_key: String,
  binance_api_secret: String,
  use_testnet: bool,
}

impl UserConfig {
  pub fn to_config(&self) -> ExchangeConfig {
    ExchangeConfig {
      binance_api_key: self.binance_api_key.clone(),
      binance_api_secret: self.binance_api_secret.clone(),
      use_testnet: self.use_testnet,
    }
  }
}

#[derive(serde::Deserialize, Debug)]
pub struct ExchangeConfig {
  pub binance_api_key: String,
  pub binance_api_secret: String,
  pub use_testnet: bool,
}

impl ExchangeConfig {
  pub fn get_exchange_stream_url(use_testnet: bool) -> String {
    let binance_stream_url = if use_testnet {
      "wss://testnet.binance.vision/ws".to_string()
    } else {
      "wss://stream.binance.com:9443/ws".to_string()
    };
    binance_stream_url
  }

  pub fn get_exchange_url(use_testnet: bool) -> String {
    let binance_url = if use_testnet {
      "https://testnet.binance.vision".to_string()
    } else {
      "https://api.binance.com".to_string()
    };
    binance_url
  }
}

#[derive(Error, Debug)]
pub enum ConfigError {
  #[error(
    "Problem opening config file, make sure configuration exists at `.config/env.toml`."
  )]
  ReadError,
  #[error("Problem setting configuration")]
  SetError,
}
pub fn read_config() -> Result<ExchangeConfig, ConfigError> {
  let config_file =
    std::fs::read_to_string(".config/env.toml").map_err(|_| ConfigError::ReadError)?;
  let user_config: UserConfig =
    toml::from_str(&config_file).map_err(|_| ConfigError::SetError)?;
  let config = user_config.to_config();
  Ok(config)
}
