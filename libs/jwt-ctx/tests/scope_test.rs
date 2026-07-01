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

#[tokio::test]
async fn extracts_tenant_scope_from_jwt() {
    let req = req_with_tid(42);
    let (mut parts, _) = req.into_parts();
    let scope = TenantScope::from_request_parts(&mut parts, &())
        .await
        .expect("extracts");
    assert_eq!(scope.0, 42);
}

#[tokio::test]
async fn missing_auth_returns_unauthorized() {
    let req: Request<()> = Request::new(());
    let (mut parts, _) = req.into_parts();
    let result = TenantScope::from_request_parts(&mut parts, &()).await;
    assert_eq!(result.unwrap_err(), axum::http::StatusCode::UNAUTHORIZED);
}
