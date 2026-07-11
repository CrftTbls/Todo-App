//! エラーモデルの定義。パニックを排除し、すべてのエラーパスをResultでハンドリングする。

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database connection pool error: {0}")]
    PoolError(#[from] r2d2::Error),

    #[error("Database query error: {0}")]
    DbError(#[from] rusqlite::Error),

    #[error("Serialization/Deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("UI communication error: {0}")]
    UiCommunicationError(String),

    #[error("Channel send error: {0}")]
    ChannelSendError(String),

    #[error("Path resolution error: {0}")]
    PathError(String),

    #[error("Internal logic error: {0}")]
    InternalError(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
