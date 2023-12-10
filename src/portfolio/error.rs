use thiserror::Error;

use crate::database::error::DatabaseError;

/// All errors generated in the barter::portfolio module.
#[derive(Error, Debug)]
pub enum PortfolioError {
    #[error("Failed to build portfolio due to missing attributes: {0}")]
    BuilderIncomplete(&'static str),
    #[error("Cannot generate PositionExit from Position that has not been exited")]
    PositionExit,
    #[error("Failed to interact with database: {0}")]
    RepositoryInteraction(#[from] DatabaseError),
    #[error("Cannot exit Position with an entry decision FillEvent.")]
    CannotExitPositionWithEntryFill,
    #[error("Cannot exit Position with an entry decision FillEvent.")]
    CannotEnterPositionWithExitFill,
    #[error("Failed to parse Position entry Side due to ambiguous fill quantity & Decision.")]
    ParseEntrySide,
}
