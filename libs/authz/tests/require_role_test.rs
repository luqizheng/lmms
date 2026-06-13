use authz::require_role;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::middleware as axum_mw;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

async fn secret(_: jwt_ctx::Principal) -> &'static str {
    "ok"
}

#[tokio::test]
async fn with_role_passes() {
    let app = Router::new()
        .route("/", get(secret))
        .layer(axum_mw::from_fn(require_role("admin")));
    let req = Request::builder()
        .uri("/")
        .header(header::HeaderName::from_static("x-user-id"), "u")
        .header(
            header::HeaderName::from_static("x-user-roles"),
            "user,admin",
        )
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn without_role_is_403() {
    let app = Router::new()
        .route("/", get(secret))
        .layer(axum_mw::from_fn(require_role("admin")));
    let req = Request::builder()
        .uri("/")
        .header(header::HeaderName::from_static("x-user-id"), "u")
        .header(header::HeaderName::from_static("x-user-roles"), "user")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
