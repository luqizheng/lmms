pub use common_error::{AppError, AppResult, ErrorKind};

pub trait AppErrorExt {
    fn config_not_found(msg: impl Into<String>) -> Self;
    fn namespace_not_found(msg: impl Into<String>) -> Self;
    fn validation_failed(msg: impl Into<String>) -> Self;
}

impl AppErrorExt for AppError {
    fn config_not_found(msg: impl Into<String>) -> Self {
        Self::not_found("CFG_ITEM_1001", msg)
    }
    fn namespace_not_found(msg: impl Into<String>) -> Self {
        Self::not_found("CFG_NS_2001", msg)
    }
    fn validation_failed(msg: impl Into<String>) -> Self {
        Self::bad_request("CFG_VALID_3001", msg)
    }
}
