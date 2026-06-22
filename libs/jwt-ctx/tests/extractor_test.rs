use axum::body::Body;
use axum::http::{header, Request};
use axum::routing::get;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use jwt_ctx::Principal;
use tower::ServiceExt;

async fn who(me: Principal) -> String {
    format!("u{}", me.user_code)
}

fn create_test_jwt(payload: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode("{\"alg\":\"HS256\",\"typ\":\"JWT\"}");
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload);
    format!("{}.{}.signature", header, payload_b64)
}

#[tokio::test]
async fn extractor_returns_principal() {
    let payload = r#"{"sub":"u","user_code":"aB3xY9zQ"}"#;
    let token = create_test_jwt(payload);
    
    let app = Router::new().route("/", get(who));
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
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