use file_service::error::{AppError, AppErrorExt};

#[test]
fn file_too_large_returns_2002() {
    let err = AppError::file_too_large("file too large");
    assert_eq!(err.code(), "FILE_UPLOAD_2002");
}

#[test]
fn file_not_found_returns_1001() {
    let err = AppError::file_not_found("not found");
    assert_eq!(err.code(), "FILE_ITEM_1001");
}

#[test]
fn file_upload_failed_returns_2001() {
    let err = AppError::file_upload_failed("upload failed");
    assert_eq!(err.code(), "FILE_UPLOAD_2001");
}

#[test]
fn presign_expired_returns_3001() {
    let err = AppError::presign_expired("expired");
    assert_eq!(err.code(), "FILE_PRESIGN_3001");
}