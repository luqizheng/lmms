pub use digua_common_error::{AppError, AppResult, ErrorKind};

pub trait AppErrorExt {
    fn audit_not_found(msg: impl Into<String>) -> Self;
    fn export_failed(msg: impl Into<String>) -> Self;
}

impl AppErrorExt for AppError {
    fn audit_not_found(msg: impl Into<String>) -> Self {
        Self::not_found("AUD_LOG_1001", msg)
    }
    fn export_failed(msg: impl Into<String>) -> Self {
        Self::internal("AUD_LOG_1003", msg)
    }
}
