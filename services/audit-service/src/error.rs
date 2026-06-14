pub use common_error::{AppError, AppResult, ErrorKind};

impl AppError {
    pub fn audit_not_found(msg: impl Into<String>) -> Self {
        Self::not_found("AUD_LOG_1001", msg)
    }
    pub fn export_failed(msg: impl Into<String>) -> Self {
        Self::internal("AUD_LOG_1003", msg)
    }
}
