use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Failed to serialize/deserialize JSON due to: {0}")]
    JsonSerDe(#[from] serde_json::Error),
    #[error("SQL error: {0}")]
    SQLError(#[from] sqlx::Error),
    #[error("Failed to read from database")]
    ReadError,
    #[error("Data was not found in the database")]
    DataMissing,
    #[error("Database initialization problem: {0}")]
    Initialization(String),
}
