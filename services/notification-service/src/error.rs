use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("redis: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("upstream [{code}]: {message}")]
    External { code: String, message: String },
}

impl AppError {
    pub fn external(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::External {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match &self {
            AppError::NotFound => {
                (StatusCode::NOT_FOUND, Json(json!({ "error": "not_found" }))).into_response()
            }
            AppError::Forbidden => {
                (StatusCode::FORBIDDEN, Json(json!({ "error": "forbidden" }))).into_response()
            }
            AppError::BadRequest(m) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": m }))).into_response()
            }
            AppError::Db(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("db: {e}") })),
            )
                .into_response(),
            AppError::Redis(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("redis: {e}") })),
            )
                .into_response(),
            AppError::Internal(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("{e}") })),
            )
                .into_response(),
            AppError::External { code, message } => (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "upstream", "code": code, "message": message })),
            )
                .into_response(),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
