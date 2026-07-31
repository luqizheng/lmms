pub use digua_common_error::{AppError, AppResult, ErrorKind};

pub trait AppErrorExt {
    fn file_not_found(msg: impl Into<String>) -> Self;
    fn file_upload_failed(msg: impl Into<String>) -> Self;
    fn file_too_large(msg: impl Into<String>) -> Self;
    fn presign_expired(msg: impl Into<String>) -> Self;
}

impl AppErrorExt for AppError {
    fn file_not_found(msg: impl Into<String>) -> Self {
        Self::not_found("FILE_ITEM_1001", msg)
    }
    fn file_upload_failed(msg: impl Into<String>) -> Self {
        Self::internal("FILE_UPLOAD_2001", msg)
    }
    fn file_too_large(msg: impl Into<String>) -> Self {
        Self::bad_request("FILE_UPLOAD_2002", msg)
    }
    fn presign_expired(msg: impl Into<String>) -> Self {
        Self::unauthorized("FILE_PRESIGN_3001", msg)
    }
}
