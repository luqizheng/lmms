use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("etag conflict: draft was modified by someone else")]
    EtagConflict {
        current_etag: String,
        updated_at: DateTime<Utc>,
    },
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Serialize)]
struct ErrorBody {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, data) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, 404, "not found".to_string(), None),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, 400, msg, None),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, 409, msg, None),
            AppError::EtagConflict {
                current_etag,
                updated_at,
            } => (
                StatusCode::CONFLICT,
                409,
                "draft was modified by someone else".to_string(),
                Some(json!({
                    "current_etag": current_etag,
                    "updated_at": updated_at,
                })),
            ),
            AppError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                500,
                "internal error".to_string(),
                None,
            ),
        };

        (
            status,
            Json(ErrorBody {
                code,
                message,
                data,
            }),
        )
            .into_response()
    }
}
