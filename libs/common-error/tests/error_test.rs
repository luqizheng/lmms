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