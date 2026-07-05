use axum::extract::FromRequestParts;
use axum::http::{HeaderValue, Request};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use jwt_ctx::TenantScope;

fn req_with_tid(tid: i64) -> Request<()> {
    let header_b64 = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let payload_b64 = URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&serde_json::json!({"sub":"u","tid":tid})).unwrap());
    let token = format!("{}.{}.sig", header_b64, payload_b64);
    let mut req = Request::new(());
    req.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    req
}

fn req_with_cross_tenant() -> Request<()> {
    let header_b64 = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let payload_b64 = URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&serde_json::json!({"sub":"u","tid":42,"ct":true})).unwrap());
    let token = format!("{}.{}.sig", header_b64, payload_b64);
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
    let req = req_with_tid(42);
    let (mut parts, _) = req.into_parts();
    let scope = TenantScope::from_request_parts(&mut parts, &())
        .await
        .expect("extracts");
    assert_eq!(scope, TenantScope::Single(42));
}

#[tokio::test]
async fn extracts_cross_tenant_when_ct_true() {
    let req = req_with_cross_tenant();
    let (mut parts, _) = req.into_parts();
    let scope = TenantScope::from_request_parts(&mut parts, &())
        .await
        .expect("extracts");
    assert_eq!(scope, TenantScope::CrossTenant);
    assert_eq!(scope.tenant_id(), None);
}

#[tokio::test]
async fn missing_auth_returns_unauthorized() {
    let req: Request<()> = Request::new(());
    let (mut parts, _) = req.into_parts();
    let result = TenantScope::from_request_parts(&mut parts, &()).await;
    assert_eq!(result.unwrap_err(), axum::http::StatusCode::UNAUTHORIZED);
}
