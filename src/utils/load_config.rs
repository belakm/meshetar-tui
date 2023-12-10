use thiserror::Error;

#[derive(serde::Deserialize)]
pub struct Config {
    pub binance_api_key: String,
    pub binance_api_secret: String,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Problem opening config file")]
    ReadError,
    #[error("Problem setting configuration")]
    SetError,
}
// TODO: ditch the unwraps and emit ConfigError instead
pub fn read_config() -> Result<Config, ConfigError> {
    let config_file = std::fs::read_to_string("config.toml").map_err(|_| ConfigError::ReadError)?;
    let config: Config = toml::from_str(&config_file).map_err(|_| ConfigError::SetError)?;
    Ok(config)
}
