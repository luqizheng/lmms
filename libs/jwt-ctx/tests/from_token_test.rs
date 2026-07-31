//! Phase 5-B: tests for `Principal::from_token` (async, with verify).

use digua_common_jwt::{sign_token, JWKSCache};
use digua_jwt_ctx::Principal;
use std::time::Duration;

const TEST_PRIVATE_PEM: &str =
    include_str!("../../digua_common-jwt/tests/fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str = include_str!("../../digua_common-jwt/tests/fixtures/test_public.pem");

fn mock_jwks() -> JWKSCache {
    JWKSCache::from_pem(TEST_PUBLIC_PEM).expect("test public key")
}

#[tokio::test]
async fn from_token_with_valid_signature_succeeds() {
    let cache = mock_jwks();
    let claims = serde_json::json!({
        "iss": "token-service",
        "sub": "user-1",
        "tid": 1,
    });
    let token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let p = Principal::from_token(&token, &cache, None).await.unwrap();
    assert_eq!(p.user_id, "user-1");
    assert_eq!(p.tenant_id, 1);
}

#[tokio::test]
async fn from_token_with_expected_aud_succeeds() {
    let cache = mock_jwks();
    let claims = serde_json::json!({
        "sub": "service-x",
        "tid": 1,
        "aud": "iam-service",
    });
    let token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let p = Principal::from_token(&token, &cache, Some("iam-service")).await.unwrap();
    assert_eq!(p.user_id, "service-x");
}

#[tokio::test]
async fn from_token_with_tampered_signature_fails() {
    let cache = mock_jwks();
    let claims = serde_json::json!({
        "sub": "u",
        "tid": 1,
    });
    let mut token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let mut parts: Vec<&str> = token.split('.').collect();
    let sig = parts[2].to_string();
    let flipped = sig
        .chars()
        .next()
        .map(|c| if c == 'A' { 'B' } else { 'A' })
        .unwrap();
    let new_sig = format!("{}{}", flipped, &sig[1..]);
    parts[2] = &new_sig;
    token = parts.join(".");

    let err = Principal::from_token(&token, &cache, None).await.unwrap_err();
    assert!(matches!(err, digua_jwt_ctx::PrincipalVerifyError::VerifyFailed(_)));
}
