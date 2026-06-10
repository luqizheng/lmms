use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("forbidden")]
    Forbidden,
    #[error("unauthorized")]
    Unauthorized,
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Validation(_) => "VALIDATION",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Conflict(_) => "CONFLICT",
            AppError::Forbidden => "FORBIDDEN",
            AppError::Unauthorized => "UNAUTHORIZED",
            AppError::Db(sqlx::Error::RowNotFound) => "NOT_FOUND",
            AppError::Db(_) => "DB_ERROR",
            AppError::Internal(_) => "INTERNAL",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Db(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "code": self.code(),
            "message": self.to_string(),
        }));
        (self.status(), body).into_response()
    }
}
