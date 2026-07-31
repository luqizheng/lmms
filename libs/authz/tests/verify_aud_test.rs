use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware,
    response::IntoResponse,
    routing::get,
    Router,
};
use digua_authz::verify_aud_principal_middleware;
use digua_common_jwt::{sign_token, JWKSCache};
use digua_jwt_ctx::PrincipalStateProvider;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const TEST_PRIVATE_PEM: &str =
    include_str!("../../digua_common-jwt/tests/fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str =
    include_str!("../../digua_common-jwt/tests/fixtures/test_public.pem");

#[derive(Clone)]
struct State {
    jwks: Arc<JWKSCache>,
}

impl PrincipalStateProvider for State {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}

fn state() -> State {
    State {
        jwks: Arc::new(JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap()),
    }
}

async fn hello() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

fn make_app(aud: &'static str) -> Router {
    Router::new()
        .route("/", get(hello))
        .route_layer(middleware::from_fn_with_state(
            state(),
            verify_aud_principal_middleware::<State>(aud),
        ))
        .with_state(state())
}

fn make_token(aud: &str) -> String {
    sign_token(
        TEST_PRIVATE_PEM,
        &serde_json::json!({
            "sub": "service-x",
            "tid": 1,
            "aud": aud,
        }),
        Duration::from_secs(60),
    )
    .unwrap()
}

#[tokio::test]
async fn valid_aud_passes_through() {
    let app = make_app("iam-service");
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {}", make_token("iam-service")))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_aud_returns_401() {
    let app = make_app("iam-service");
    let req = Request::builder()
        .uri("/")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", make_token("file-service")),
        )
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn missing_auth_returns_401() {
    let app = make_app("iam-service");
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bad_signature_returns_401() {
    let app = make_app("iam-service");
    let mut token = make_token("iam-service");
    let parts: Vec<&str> = token.split('.').collect();
    let new_sig = format!("A{}", &parts[2][1..]);
    token = format!("{}.{}.{}", parts[0], parts[1], new_sig);
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
