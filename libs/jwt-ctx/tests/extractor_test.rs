use axum::body::Body;
use axum::http::{header, Request};
use axum::routing::get;
use axum::Router;
use digua_common_jwt::{sign_token, JWKSCache};
use digua_jwt_ctx::{Principal, PrincipalStateProvider};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const TEST_PRIVATE_PEM: &str =
    include_str!("../../digua_common-jwt/tests/fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str = include_str!("../../digua_common-jwt/tests/fixtures/test_public.pem");
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

// ============================================================================
// 5B-fu6: 响应 body 不泄露 JWT 内部错误
// 对 6 类错误路径都断言 status == 401 && body.is_empty()
// ============================================================================

#[tokio::test]
async fn bad_signature_body_is_empty() {
    let state = make_state();
    let mut token = make_token(serde_json::json!({"sub":"u","tid":1}));
    let parts: Vec<&str> = token.split('.').collect();
    let new_sig = format!("A{}", &parts[2][1..]);
    token = format!("{}.{}.{}", parts[0], parts[1], new_sig);
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn expired_token_body_is_empty() {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    let state = make_state();
    // sign_token 强制 exp = now + ttl，不能产生「已过期」token；
    // 必须用 jsonwebtoken 直接构造，exp 设为 1 小时前。
    let now = chrono::Utc::now().timestamp();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("default".into());
    let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes()).unwrap();
    let claims = serde_json::json!({
        "iss": "test",
        "sub": "u",
        "tid": 1,
        "iat": now - 3600,
        "exp": now - 3600,
    });
    let token = encode(&header, &claims, &key).unwrap();
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn missing_sub_claim_body_is_empty() {
    let state = make_state();
    let token = make_token(serde_json::json!({"tid": 1}));
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn missing_tid_claim_body_is_empty() {
    let state = make_state();
    let token = make_token(serde_json::json!({"sub": "u"}));
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn wrong_signing_key_body_is_empty() {
    // 用 other_private 签，但 JWKSCache 仍用 test_public 验
    // → 签名验证失败，但 body 仍须空
    let state = make_state();
    let token = sign_token(
        OTHER_PRIVATE_PEM,
        &serde_json::json!({"sub":"u","tid":1}),
        std::time::Duration::from_secs(60),
    )
    .unwrap();
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn malformed_token_body_is_empty() {
    let state = make_state();
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, "not.a.jwt.at.all").await;
}
