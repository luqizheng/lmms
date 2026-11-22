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
    TooManyRequests,
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
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            request_id: None,
            source: None,
        }
    }
    pub fn bad_request(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::BadRequest, code, msg)
    }
    pub fn unauthorized(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unauthorized, code, msg)
    }
    pub fn forbidden(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Forbidden, code, msg)
    }
    pub fn not_found(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, code, msg)
    }
    pub fn conflict(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Conflict, code, msg)
    }
    pub fn internal(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, code, msg)
    }
    pub fn external(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::External, code, msg)
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn with_request_id(mut self, rid: impl Into<String>) -> Self {
        self.request_id = Some(rid.into());
        self
    }
    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(mut self, e: E) -> Self {
        self.source = Some(Box::new(e));
        self
    }

    /// Return a response-safe message. Internal and upstream details remain
    /// available through Display/source for server-side diagnostics, but must not
    /// be serialized to callers (they may contain URLs, hosts, SQL, or credentials).
    fn public_message(&self) -> &str {
        match self.kind {
            ErrorKind::Internal => "Internal server error",
            ErrorKind::External => "Upstream request failed",
            _ => self.message.as_str(),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::RowNotFound => {
                AppError::not_found("DB_NOT_FOUND", "row not found").with_source(e)
            }
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
struct ErrBody<'a> {
    code: &'a str,
    message: &'a str,
    request_id: Option<&'a str>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.kind {
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::External => StatusCode::BAD_GATEWAY,
        };
        let body = ErrBody {
            code: &self.code,
            message: self.public_message(),
            request_id: self.request_id.as_deref(),
        };
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;

pub mod codes;


#[cfg(test)]
mod response_safety_tests {
    use super::*;
    use axum::body::{to_bytes, Body};

    async fn response_json(error: AppError) -> serde_json::Value {
        let response = error.into_response();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read error response body");
        serde_json::from_slice(&body).expect("decode error response JSON")
    }

    #[tokio::test]
    async fn internal_details_are_not_serialized_to_http_clients() {
        let secret = "postgres://admin:super-secret@db.internal/app";
        let body = response_json(AppError::internal(
            "DB_CONNECT",
            format!("connection failed: {secret}"),
        )).await;

        assert_eq!(body["code"], "DB_CONNECT");
        assert_eq!(body["message"], "Internal server error");
        assert!(!body.to_string().contains(secret));
    }

    #[tokio::test]
    async fn upstream_details_are_not_serialized_to_http_clients() {
        let secret = "http://token-service.internal/path?token=super-secret";
        let body = response_json(AppError::external(
            "HTTP_ERR",
            format!("request to {secret} failed"),
        )).await;

        assert_eq!(body["code"], "HTTP_ERR");
        assert_eq!(body["message"], "Upstream request failed");
        assert!(!body.to_string().contains(secret));
    }

    #[tokio::test]
    async fn client_error_messages_remain_actionable_in_http_response() {
        let body = response_json(AppError::bad_request(
            "INVALID_INPUT",
            "name is required",
        )).await;

        assert_eq!(body["message"], "name is required");
    }
}
