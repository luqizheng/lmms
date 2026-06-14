pub use common_error::{AppError, AppResult, ErrorKind};

impl AppError {
    pub fn config_not_found(msg: impl Into<String>) -> Self {
        Self::not_found("CFG_ITEM_1001", msg)
    }
    pub fn namespace_not_found(msg: impl Into<String>) -> Self {
        Self::not_found("CFG_NS_2001", msg)
    }
    pub fn validation_failed(msg: impl Into<String>) -> Self {
        Self::bad_request("CFG_VALID_3001", msg)
    }
}
