use thiserror::Error;

#[derive(Debug, Error)]
pub enum KudiHttpError {
    #[error("auth failed (HTTP {status}): {message}")]
    Auth {
        status: u16,
        message: String,
        details: Option<String>,
    },
    #[error("validation failed (HTTP {status}): {message}")]
    Validation {
        status: u16,
        message: String,
        details: Option<String>,
    },
    #[error("rate limited (HTTP 429): {message}")]
    RateLimit {
        message: String,
        details: Option<String>,
    },
    #[error("server error (HTTP {status}): {message}")]
    Server {
        status: u16,
        message: String,
        details: Option<String>,
    },
    #[error("network error: {0}")]
    Network(String),
}

pub type AuthError = KudiHttpError;
pub type ServerError = KudiHttpError;
pub type NetworkError = KudiHttpError;
pub type ValidationError = KudiHttpError;
pub type RateLimitError = KudiHttpError;
