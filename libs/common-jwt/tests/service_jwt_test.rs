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

/// 2026-11-07 multi-tenant mint: per-tenant tokens must mint via HTTP per
/// (target, tid, ct) and never satisfy each other from the cache.
#[tokio::test]
async fn get_for_tenant_isolates_cache_keys() {
    let c = cache();

    // Different (tid, ct) combos each hit the (unreachable) token-service —
    // proving no cross-key cache reuse and that a per-tenant call never
    // resolves via the legacy fixed-tenant entry.
    let c2 = std::sync::Arc::new(c);
    let c3 = c2.clone();
    let (a, b, d) = tokio::join!(
        c2.get_for_tenant("file-service", 7, false),
        c3.get_for_tenant("file-service", 8, false),
        c2.get_for_tenant("file-service", 7, true),
    );
    assert!(a.is_err() && b.is_err() && d.is_err());

    // primed legacy entry must not satisfy per-tenant lookups either
    let c = cache();
    c.prime_cache("file-service", Duration::from_secs(300))
        .await
        .unwrap();
    let got = c.get("file-service").await; // legacy key resolves from cache (no HTTP)
    assert!(got.is_ok());
    let err = c
        .get_for_tenant("file-service", 7, false)
        .await
        .unwrap_err();
    assert!(matches!(err, ServiceJwtError::TokenServiceUnavailable(_)));
}

/// Regression (2026-11-07, caught by pm-api integration): the FIRST mint for
/// a (target,tid) must POST aud=<target>/tid=<tid> — the params lookup used
/// to fall back to the composite cache key as the audience with the config
/// tenant, producing aud="file-service:7", tid=1.
#[tokio::test]
async fn get_for_tenant_mints_explicit_aud_and_tid() {
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/service-jwt"))
        .and(header("authorization", "Bearer bt-test"))
        .and(body_partial_json(
            serde_json::json!({"aud": "file-service", "tid": 7}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"access_token": "svc-jwt-1", "expires_in": 3600}),
            ),
        )
        .expect(1)
        .mount(&server)
        .await;

    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let c = ServiceJwtCache::new(
        ServiceJwtConfig {
            token_service_url: server.uri(),
            bootstrap_token: "bt-test".into(),
            caller_service_name: "test-svc".into(),
            caller_tenant: 1,
            refresh_skew: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        },
        http,
        TEST_PRIVATE_PEM.to_string(),
    );
    let tok = c.get_for_tenant("file-service", 7, false).await.unwrap();
    assert_eq!(tok, "svc-jwt-1");
    // Second call must resolve from cache without another mint.
    let tok2 = c.get_for_tenant("file-service", 7, false).await.unwrap();
    assert_eq!(tok2, "svc-jwt-1");
    server.verify().await;
}
