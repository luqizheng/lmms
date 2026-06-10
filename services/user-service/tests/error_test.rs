use user_service::error::AppError;
use axum::http::StatusCode;
use axum::response::IntoResponse;

#[test]
fn validation_maps_to_400() {
    let err = AppError::Validation("bad".into());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn not_found_maps_to_404() {
    let err = AppError::NotFound("user".into());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[test]
fn forbidden_maps_to_403() {
    let err = AppError::Forbidden;
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[test]
fn db_row_not_found_maps_to_404() {
    let err = AppError::Db(sqlx::Error::RowNotFound);
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[test]
fn unauthorized_maps_to_401() {
    let err = AppError::Unauthorized;
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn conflict_maps_to_409() {
    let err = AppError::Conflict("dup".into());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
