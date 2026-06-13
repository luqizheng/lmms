use common_error::{AppError, ErrorKind};

#[test]
fn bad_request_has_correct_kind() {
    let e = AppError::bad_request("E_X", "bad");
    assert_eq!(e.kind(), ErrorKind::BadRequest);
    assert_eq!(e.code(), "E_X");
}

#[test]
fn from_sqlx_row_not_found_maps_to_not_found() {
    let sqlx_err = sqlx::Error::RowNotFound;
    let app: AppError = sqlx_err.into();
    assert_eq!(app.kind(), ErrorKind::NotFound);
}

#[test]
fn all_error_kinds_have_constructors() {
    let _ = AppError::bad_request("BR", "bad");
    let _ = AppError::unauthorized("UA", "unauth");
    let _ = AppError::forbidden("FB", "forbidden");
    let _ = AppError::not_found("NF", "not found");
    let _ = AppError::conflict("CF", "conflict");
    let _ = AppError::internal("IN", "internal");
    let _ = AppError::external("EX", "external");
}

#[test]
fn request_id_can_be_attached() {
    let e = AppError::bad_request("E", "msg").with_request_id("req-123");
    assert_eq!(e.message(), "msg");
}
