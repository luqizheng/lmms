use axum::body::Body;
use axum::http::{Request, header};
use axum::routing::get;
use axum::Router;
use jwt_ctx::Principal;
use tower::ServiceExt;

async fn who(me: Principal) -> String {
    format!("u{}", me.user_code)
}

#[tokio::test]
async fn extractor_returns_principal() {
    let app = Router::new().route("/", get(who));
    let req = Request::builder()
        .uri("/")
        .header(header::HeaderName::from_static("x-user-id"), "u")
        .header(header::HeaderName::from_static("x-user-code"), "aB3xY9zQ")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert_eq!(&body[..], b"uaB3xY9zQ");
}

#[tokio::test]
async fn missing_header_returns_401() {
    let app = Router::new().route("/", get(who));
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}
