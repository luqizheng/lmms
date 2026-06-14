pub use common_error::{AppError, AppResult, ErrorKind};

impl AppError {
    pub fn file_not_found(msg: impl Into<String>) -> Self {
        Self::not_found("FILE_ITEM_1001", msg)
    }
    pub fn file_upload_failed(msg: impl Into<String>) -> Self {
        Self::internal("FILE_UPLOAD_2001", msg)
    }
    pub fn file_too_large(msg: impl Into<String>) -> Self {
        Self::bad_request("FILE_UPLOAD_2002", msg)
    }
    pub fn presign_expired(msg: impl Into<String>) -> Self {
        Self::unauthorized("FILE_PRESIGN_3001", msg)
    }
}