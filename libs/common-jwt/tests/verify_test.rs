use digua_common_jwt::{sign_token, verify_token, JWKSCache, VerifyError};
use serde_json::json;
use std::time::Duration;

const TEST_PRIVATE_PEM: &str = include_str!("fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str = include_str!("fixtures/test_public.pem");

fn mock_jwks() -> JWKSCache {
    JWKSCache::from_pem(TEST_PUBLIC_PEM).expect("test public key must parse")
}

#[tokio::test]
async fn sign_then_verify_roundtrip_succeeds() {
    let cache = mock_jwks();
    let claims = json!({
        "iss": "test",
        "sub": "user-1",
        "aud": "iam-service",
        "tid": 1,
    });
    let token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let verified = verify_token(&token, &cache, Some("iam-service"))
        .await
        .unwrap();
    assert_eq!(verified["sub"], "user-1");
    assert_eq!(verified["aud"], "iam-service");
}

#[tokio::test]
async fn expired_token_returns_expired_error() {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    let cache = mock_jwks();
    // Build a token with exp 1 hour in the past. sign_token always sets
    // exp = now + ttl; to test "already expired" we must construct the
    // token directly with jsonwebtoken (bypassing our wrapper).
    let now = chrono::Utc::now().timestamp();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("default".into());
    let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes()).unwrap();
    let claims = json!({
        "iss": "test",
        "sub": "user-1",
        "aud": "iam-service",
        "tid": 1,
        "iat": now - 3600,
        "exp": now - 3600,
    });
    let token = encode(&header, &claims, &key).unwrap();
    let err = verify_token(&token, &cache, Some("iam-service"))
        .await
        .unwrap_err();
    assert!(matches!(err, VerifyError::Expired), "got {:?}", err);
}

#[tokio::test]
async fn wrong_audience_returns_mismatch() {
    let cache = mock_jwks();
    let now = chrono::Utc::now().timestamp();
    let claims = json!({
        "iss": "test",
        "sub": "user-1",
        "aud": "file-service",
        "tid": 1,
        "exp": now + 60,
    });
    let token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let err = verify_token(&token, &cache, Some("iam-service"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, VerifyError::AudienceMismatch { .. }),
        "got {:?}",
        err
    );
}

#[tokio::test]
async fn tampered_signature_returns_invalid_signature() {
    let cache = mock_jwks();
    let now = chrono::Utc::now().timestamp();
    let claims = json!({
        "iss": "test",
        "sub": "user-1",
        "aud": "iam-service",
        "tid": 1,
        "exp": now + 60,
    });
    let token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    // Tamper: flip a byte in the signature segment.
    let parts: Vec<&str> = token.split('.').collect();
    let sig = parts[2].to_string();
    let flipped_char = sig
        .chars()
        .next()
        .map(|c| if c == 'A' { 'B' } else { 'A' })
        .unwrap();
    let new_sig = format!("{}{}", flipped_char, &sig[1..]);
    let tampered = format!("{}.{}.{}", parts[0], parts[1], new_sig);
    let err = verify_token(&tampered, &cache, Some("iam-service"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, VerifyError::SignatureInvalid),
        "got {:?}",
        err
    );
}

#[tokio::test]
async fn unknown_kid_returns_malformed() {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("attacker-controlled".into());
    let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes()).unwrap();
    let now = chrono::Utc::now().timestamp();
    let claims = json!({
        "iss": "test",
        "sub": "user-1",
        "aud": "iam-service",
        "tid": 1,
        "exp": now + 60,
    });
    let token = encode(&header, &claims, &key).unwrap();
    let cache = mock_jwks();
    let err = verify_token(&token, &cache, Some("iam-service"))
        .await
        .unwrap_err();
    assert!(matches!(err, VerifyError::Malformed(_)), "got {:?}", err);
}
