use thiserror::Error;

use crate::database::error::DatabaseError;

#[derive(Error, Debug)]
pub enum CoreError {
  #[error("Failed to build core due to missing attributes: {0}")]
  BuilderIncomplete(&'static str),
  #[error("Failed to interact with database: {0}")]
  RepositoryInteraction(#[from] DatabaseError),
  #[error("Failed to build report: {0}")]
  ReportError(String),
}
