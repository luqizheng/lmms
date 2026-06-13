pub use common_error::{AppError, AppResult, ErrorKind};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommonError {
    #[error("JWT error: {0}")]
    JwtError(String),

    #[error("Invalid token claims")]
    InvalidClaims,

    #[error("Token expired")]
    TokenExpired,

    #[error("Token revoked")]
    TokenRevoked,

    #[error("Unknown error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, CommonError>;
