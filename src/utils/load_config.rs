use thiserror::Error;

#[derive(serde::Deserialize, Debug)]
pub struct Config {
  pub binance_api_key: String,
  pub binance_api_secret: String,
  pub binance_url: String,
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
// TODO: ditch the unwraps and emit ConfigError instead
pub fn read_config() -> Result<Config, ConfigError> {
  let config_file =
    std::fs::read_to_string(".config/env.toml").map_err(|_| ConfigError::ReadError)?;
  let config: Config = toml::from_str(&config_file).map_err(|_| ConfigError::SetError)?;
  Ok(config)
}
