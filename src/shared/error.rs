use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Database pool error: {0}")]
    Pool(#[from] deadpool_sqlite::PoolError),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Telegram API error: {0}")]
    Telegram(String),

    #[error("AI API error: {0}")]
    AiApi(String),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::Validation(_) | AppError::UnknownTool(_) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            AppError::Telegram(_) | AppError::AiApi(_) => {
                (StatusCode::BAD_GATEWAY, self.to_string())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        error!("[AppError] HTTP {status}: {error_message}");

        (status, error_message).into_response()
    }
}
