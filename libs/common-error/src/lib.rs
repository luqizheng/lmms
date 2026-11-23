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
    use axum::body::to_bytes;

    async fn response_parts(error: AppError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read error response body");
        let json = serde_json::from_slice(&body).expect("decode error response JSON");
        (status, json)
    }

    #[tokio::test]
    async fn internal_details_are_not_serialized_to_http_clients() {
        let secret = "postgres://admin:super-secret@db.internal/app";
        let error = AppError::internal("DB_CONNECT", format!("connection failed: {secret}"))
            .with_request_id("req-internal-1");
        let (status, body) = response_parts(error).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "DB_CONNECT");
        assert_eq!(body["message"], "Internal server error");
        assert_eq!(body["request_id"], "req-internal-1");
        assert!(!body.to_string().contains(secret));
    }

    #[tokio::test]
    async fn upstream_details_are_not_serialized_to_http_clients() {
        let secret = "http://token-service.internal/path?token=super-secret";
        let error = AppError::external("HTTP_ERR", format!("request to {secret} failed"));
        let (status, body) = response_parts(error).await;

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["code"], "HTTP_ERR");
        assert_eq!(body["message"], "Upstream request failed");
        assert!(!body.to_string().contains(secret));
    }

    #[tokio::test]
    async fn reqwest_error_conversion_is_sanitized_in_http_response() {
        // Real offline reqwest::Error: loopback port with no listener refuses
        // instantly, and the error Display carries the full URL including query.
        let secret = "http://127.0.0.1:1/upstream?token=super-secret";
        let client = reqwest::Client::new();
        let send_err = client
            .get(secret)
            .send()
            .await
            .expect_err("loopback request must fail");
        assert!(
            send_err.to_string().contains(secret),
            "precondition: reqwest Display should carry the URL"
        );

        let (status, body) = response_parts(AppError::from(send_err)).await;

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["code"], "HTTP_ERR");
        assert_eq!(body["message"], "Upstream request failed");
        assert!(!body.to_string().contains(secret));
    }

    #[tokio::test]
    async fn sqlx_error_conversion_is_sanitized_in_http_response() {
        let secret = "postgres://admin:super-secret@db.internal/app";
        let source = sqlx::Error::Configuration(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to connect: {secret}"),
        )));
        let (status, body) = response_parts(AppError::from(source)).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "DB_ERR");
        assert_eq!(body["message"], "Internal server error");
        assert!(!body.to_string().contains(secret));
    }

    #[tokio::test]
    async fn sqlx_row_not_found_keeps_client_actionable_message() {
        let (status, body) = response_parts(AppError::from(sqlx::Error::RowNotFound)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "DB_NOT_FOUND");
        assert_eq!(body["message"], "row not found");
    }

    #[tokio::test]
    async fn client_error_messages_remain_actionable_in_http_response() {
        let error = AppError::bad_request("INVALID_INPUT", "name is required");
        let (status, body) = response_parts(error).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_INPUT");
        assert_eq!(body["message"], "name is required");
    }

    #[test]
    fn internal_details_remain_visible_via_display_and_source_chain() {
        let secret = "postgres://admin:super-secret@db.internal/app";
        let source = sqlx::Error::Configuration(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to connect: {secret}"),
        )));
        let error = AppError::internal("DB_ERR", "database error").with_source(source);

        assert_eq!(error.to_string(), "[DB_ERR] database error");
        let src = std::error::Error::source(&error).expect("source chain present");
        assert!(src.to_string().contains(secret));
    }
}
