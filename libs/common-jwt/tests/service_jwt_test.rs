use digua_common_jwt::{
    verify_token, JWKSCache, ServiceJwtCache, ServiceJwtConfig, ServiceJwtError,
};
use std::time::Duration;

const TEST_PRIVATE_PEM: &str = include_str!("fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str = include_str!("fixtures/test_public.pem");

fn jwks() -> JWKSCache {
    JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap()
}

fn cache() -> ServiceJwtCache {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    ServiceJwtCache::new(
        ServiceJwtConfig {
            token_service_url: "http://localhost:9999".into(),
            bootstrap_token: "bt-test".into(),
            caller_service_name: "test-svc".into(),
            caller_tenant: 1,
            refresh_skew: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        },
        http,
        TEST_PRIVATE_PEM.to_string(),
    )
}

#[tokio::test]
async fn sign_local_roundtrip_works_for_service_jwt() {
    // Use the local-sign path (production is HTTP; tests use local for speed).
    let c = cache();
    let jwt = c
        .sign_local("iam-service", Duration::from_secs(120))
        .unwrap();
    // Verify using digua_common_jwt (with the matching public key).
    let jwks = jwks();
    let verified = verify_token(&jwt, &jwks, Some("iam-service"))
        .await
        .unwrap();
    assert_eq!(verified["aud"], "iam-service");
    assert_eq!(verified["sub"], "test-svc");
    assert_eq!(verified["roles"][0], "service");
}

#[tokio::test]
async fn get_calls_http_when_cache_empty() {
    // Use a ServiceJwtCache whose http client will hit an unreachable URL.
    // We expect TokenServiceUnavailable error from the post, not a panic.
    let c = cache();
    let err = c.get("iam-service").await.unwrap_err();
    assert!(matches!(err, ServiceJwtError::TokenServiceUnavailable(_)));
}

#[tokio::test]
async fn refresh_failure_propagates() {
    let c = cache();
    let err = c.refresh("iam-service").await.unwrap_err();
    assert!(err.to_string().contains("token-service") || err.to_string().contains("connection"));
}
