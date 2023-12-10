use crate::{
    assets::error::AssetError, portfolio::error::PortfolioError, strategy::error::StrategyError,
};
use thiserror::Error;

/// All errors generated in the barter::portfolio module.
#[derive(Error, Debug)]
pub enum TraderError {
    #[error("Failed to build trader due to missing attributes: {0}")]
    BuilderIncomplete(&'static str),
    #[error("Failed to build fill event due to missing attributes: {0}")]
    FillBuilderIncomplete(&'static str),
    #[error("Failed to interact with Portfolio")]
    RepositoryInteraction(#[from] PortfolioError),
    #[error("Strategy error")]
    StrategyError(#[from] StrategyError),
    #[error("Asset error: {0}")]
    AssetError(#[from] AssetError),
}
