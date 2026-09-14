//! Application errors that should stop a program with a clear explanation.

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("network or file error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("JSON protocol error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ChatError>;
