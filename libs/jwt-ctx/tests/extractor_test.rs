use axum::body::Body;
use axum::http::{header, Request};
use axum::routing::get;
use axum::Router;
use common_jwt::{sign_token, JWKSCache};
use jwt_ctx::{Principal, PrincipalStateProvider};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const TEST_PRIVATE_PEM: &str = include_str!("../../common-jwt/tests/fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str = include_str!("../../common-jwt/tests/fixtures/test_public.pem");
#[allow(dead_code)]
const OTHER_PRIVATE_PEM: &str = include_str!("fixtures/other_private.pem");
#[allow(dead_code)]
const OTHER_PUBLIC_PEM: &str = include_str!("fixtures/other_public.pem");

async fn who(me: Principal) -> String {
    format!("u{}", me.user_code)
}

#[derive(Clone)]
struct State {
    jwks: Arc<JWKSCache>,
}

impl PrincipalStateProvider for State {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}

fn make_state() -> State {
    State {
        jwks: Arc::new(JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap()),
    }
}

fn make_token(payload: serde_json::Value) -> String {
    sign_token(TEST_PRIVATE_PEM, &payload, Duration::from_secs(60)).unwrap()
}

#[tokio::test]
async fn extractor_returns_principal() {
    let state = make_state();
    let token = make_token(serde_json::json!({
        "sub": "u",
        "user_code": "aB3xY9zQ",
        "tid": 1,
    }));

    let app = Router::new().route("/", get(who)).with_state(state);
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
    let state = make_state();
    let app = Router::new().route("/", get(who)).with_state(state);
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(body.is_empty(), "401 body must be empty, got: {:?}", body);
}

#[tokio::test]
async fn bad_signature_returns_401() {
    let state = make_state();
    let mut token = make_token(serde_json::json!({"sub":"u","tid":1}));
    // Tamper with the signature
    let parts: Vec<&str> = token.split('.').collect();
    let new_sig = format!("A{}", &parts[2][1..]);
    token = format!("{}.{}.{}", parts[0], parts[1], new_sig);

    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

/// 断言 401 响应且 body 为空。
/// 用法：构造一个使用 Principal extractor 的 Router，发送伪造 token，
/// 验证返回的状态码是 401 且 body bytes 为空（无 JWT 内部错误泄露）。
async fn assert_401_empty_body(app: Router, token: &str) {
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401, "expected 401 Unauthorized");
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(
        body.is_empty(),
        "401 response body must be empty (no JWT internal error leak), got: {:?}",
        body
    );
}
