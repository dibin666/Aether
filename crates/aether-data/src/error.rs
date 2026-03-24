#[derive(Debug, thiserror::Error)]
pub enum DataLayerError {
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("postgres error: {0}")]
    Postgres(#[from] sqlx::Error),

    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("operation timed out: {0}")]
    TimedOut(String),

    #[error("unexpected database value: {0}")]
    UnexpectedValue(String),
}
