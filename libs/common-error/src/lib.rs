use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Internal,
    External,
}

#[derive(Debug, Error)]
#[error("[{code}] {message}")]
pub struct AppError {
    kind: ErrorKind,
    code: String,
    message: String,
    request_id: Option<String>,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl AppError {
    pub fn new(kind: ErrorKind, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { kind, code: code.into(), message: message.into(), request_id: None, source: None }
    }
    pub fn bad_request(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::BadRequest, code, msg) }
    pub fn unauthorized(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::Unauthorized, code, msg) }
    pub fn forbidden(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::Forbidden, code, msg) }
    pub fn not_found(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::NotFound, code, msg) }
    pub fn conflict(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::Conflict, code, msg) }
    pub fn internal(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::Internal, code, msg) }
    pub fn external(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::External, code, msg) }

    pub fn kind(&self) -> ErrorKind { self.kind }
    pub fn code(&self) -> &str { &self.code }
    pub fn message(&self) -> &str { &self.message }
    pub fn with_request_id(mut self, rid: impl Into<String>) -> Self { self.request_id = Some(rid.into()); self }
    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(mut self, e: E) -> Self {
        self.source = Some(Box::new(e)); self
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::RowNotFound => AppError::not_found("DB_NOT_FOUND", "row not found").with_source(e),
            _ => AppError::internal("DB_ERR", "database error").with_source(e),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::external("HTTP_ERR", e.to_string()).with_source(e)
    }
}

#[derive(Serialize)]
struct ErrBody<'a> { code: &'a str, message: &'a str, request_id: Option<&'a str> }

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.kind {
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::External => StatusCode::BAD_GATEWAY,
        };
        let body = ErrBody { code: &self.code, message: &self.message, request_id: self.request_id.as_deref() };
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;