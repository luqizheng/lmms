use axum::extract::FromRequestParts;
use axum::http::{HeaderValue, Request};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use common_jwt::{sign_token, JWKSCache};
use jwt_ctx::{PrincipalStateProvider, TenantScope};
use std::sync::Arc;
use std::time::Duration;

const TEST_PRIVATE_PEM: &str =
    include_str!("../../common-jwt/tests/fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str =
    include_str!("../../common-jwt/tests/fixtures/test_public.pem");

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

fn req_with_tid(tid: i64) -> Request<()> {
    let token = sign_token(
        TEST_PRIVATE_PEM,
        &serde_json::json!({"sub":"u","tid":tid}),
        Duration::from_secs(60),
    )
    .unwrap();
    let mut req = Request::new(());
    req.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    req
}

fn req_with_cross_tenant() -> Request<()> {
    let token = sign_token(
        TEST_PRIVATE_PEM,
        &serde_json::json!({"sub":"u","tid":42,"ct":true}),
        Duration::from_secs(60),
    )
    .unwrap();
    let mut req = Request::new(());
    req.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    req
}

#[test]
fn single_tenant_id() {
    let s = TenantScope::Single(42);
    assert_eq!(s.tenant_id(), Some(42));
}

#[test]
fn cross_tenant_id_is_none() {
    let s = TenantScope::CrossTenant;
    assert_eq!(s.tenant_id(), None);
}

#[tokio::test]
async fn extracts_tenant_scope_from_jwt() {
    let state = make_state();
    let req = req_with_tid(42);
    let (mut parts, _) = req.into_parts();
    let scope = TenantScope::from_request_parts(&mut parts, &state)
        .await
        .expect("extracts");
    assert_eq!(scope, TenantScope::Single(42));
}

#[tokio::test]
async fn extracts_cross_tenant_when_ct_true() {
    let state = make_state();
    let req = req_with_cross_tenant();
    let (mut parts, _) = req.into_parts();
    let scope = TenantScope::from_request_parts(&mut parts, &state)
        .await
        .expect("extracts");
    assert_eq!(scope, TenantScope::CrossTenant);
    assert_eq!(scope.tenant_id(), None);
}

#[tokio::test]
async fn missing_auth_returns_unauthorized() {
    let state = make_state();
    let req: Request<()> = Request::new(());
    let (mut parts, _) = req.into_parts();
    let result = TenantScope::from_request_parts(&mut parts, &state).await;
    assert_eq!(result.unwrap_err(), axum::http::StatusCode::UNAUTHORIZED);
}
