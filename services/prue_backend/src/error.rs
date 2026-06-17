use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
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
        let (status, code, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, 404, "not found".to_string()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, 400, msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, 409, msg),
            AppError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                500,
                "internal error".to_string(),
            ),
        };

        (
            status,
            Json(ErrorBody {
                code,
                message,
                data: None,
            }),
        )
            .into_response()
    }
}
