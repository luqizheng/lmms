use jwt_simple::prelude::*;
use std::collections::{HashMap, HashSet};

fn generate_test_keypair() -> (RS256KeyPair, String) {
    let key_pair = RS256KeyPair::generate(2048).unwrap();
    let public_key_pem = key_pair.public_key().to_pem().unwrap();
    (key_pair, public_key_pem)
}

fn create_test_claims(
    subject: &str,
    issuer: &str,
    ttl_sec: i64,
) -> JWTClaims<HashMap<String, serde_json::Value>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut custom = HashMap::new();
    custom.insert(
        "scope".to_string(),
        serde_json::Value::String("access".to_string()),
    );
    custom.insert(
        "client_id".to_string(),
        serde_json::Value::String("test-client".to_string()),
    );
    custom.insert(
        "permissions".to_string(),
        serde_json::Value::Array(vec![
            serde_json::Value::String("read".to_string()),
            serde_json::Value::String("write".to_string()),
        ]),
    );

    let mut claims = JWTClaims::new();
    claims.subject = Some(subject.to_string());
    claims.jwt_id = Some("test-jti-123".to_string());
    claims.issuer = Some(issuer.to_string());
    claims.issued_at = Some(now_duration);
    claims.expires_at = Some(now_duration + Duration::from_secs(ttl_sec as u64));
    claims.custom = custom;
    claims
}

#[test]
fn test_jwt_sign_and_verify_valid_token() {
    let issuer = "test-issuer";
    let (key_pair, public_key_pem) = generate_test_keypair();

    let claims = create_test_claims("user-123", issuer, 3600);
    let token = key_pair.sign(claims).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from([issuer.to_string()]));

    let result =
        public_key.verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options));
    assert!(result.is_ok());

    let verified_claims = result.unwrap();
    assert_eq!(verified_claims.subject, Some("user-123".to_string()));
    assert_eq!(verified_claims.issuer, Some(issuer.to_string()));
}

#[test]
fn test_jwt_expired_token_rejected() {
    let issuer = "test-issuer";
    let (key_pair, public_key_pem) = generate_test_keypair();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some("user-123".to_string());
    claims.jwt_id = Some("test-jti-123".to_string());
    claims.issuer = Some(issuer.to_string());
    claims.issued_at = Some(now_duration);
    claims.expires_at = Some(now_duration - Duration::from_secs(3600));
    claims.custom = HashMap::new();

    let token = key_pair.sign(claims).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from([issuer.to_string()]));

    let result =
        public_key.verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options));
    assert!(result.is_err());
}

#[test]
fn test_jwt_wrong_issuer_rejected() {
    let (key_pair, public_key_pem) = generate_test_keypair();

    let claims = create_test_claims("user-123", "wrong-issuer", 3600);
    let token = key_pair.sign(claims).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from(["correct-issuer".to_string()]));

    let result =
        public_key.verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options));
    assert!(result.is_err());
}

#[test]
fn test_jwt_invalid_signature_rejected() {
    let (_, public_key_pem) = generate_test_keypair();
    let (wrong_key_pair, _) = generate_test_keypair();

    let claims = create_test_claims("user-123", "test-issuer", 3600);
    let token = wrong_key_pair.sign(claims).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let options = VerificationOptions::default();

    let result =
        public_key.verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options));
    assert!(result.is_err());
}

#[test]
fn test_jwt_custom_claims_extraction() {
    let issuer = "test-issuer";
    let (key_pair, public_key_pem) = generate_test_keypair();

    let mut custom = HashMap::new();
    custom.insert(
        "scope".to_string(),
        serde_json::Value::String("access".to_string()),
    );
    custom.insert(
        "client_id".to_string(),
        serde_json::Value::String("test-client".to_string()),
    );
    custom.insert(
        "permissions".to_string(),
        serde_json::Value::Array(vec![
            serde_json::Value::String("read".to_string()),
            serde_json::Value::String("write".to_string()),
        ]),
    );

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut claims = JWTClaims::new();
    claims.subject = Some("user-123".to_string());
    claims.jwt_id = Some("test-jti-123".to_string());
    claims.issuer = Some(issuer.to_string());
    claims.issued_at = Some(now_duration);
    claims.expires_at = Some(now_duration + Duration::from_secs(3600));
    claims.custom = custom;

    let token = key_pair.sign(claims).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from([issuer.to_string()]));

    let verified = public_key
        .verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options))
        .unwrap();

    let scope = verified
        .custom
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(scope, "access");

    let client_id = verified
        .custom
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(client_id, "test-client");

    let permissions = verified
        .custom
        .get("permissions")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(permissions.len(), 2);
}

#[test]
fn test_jwt_malformed_token_rejected() {
    let public_key_pem = RS256KeyPair::generate(2048)
        .unwrap()
        .public_key()
        .to_pem()
        .unwrap();
    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let options = VerificationOptions::default();

    let result = public_key
        .verify_token::<HashMap<String, serde_json::Value>>("not.a.valid.jwt", Some(options));
    assert!(result.is_err());
}

#[test]
fn test_jwt_future_token_with_accept_future() {
    let issuer = "test-issuer";
    let (key_pair, public_key_pem) = generate_test_keypair();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some("user-123".to_string());
    claims.jwt_id = Some("test-jti-123".to_string());
    claims.issuer = Some(issuer.to_string());
    claims.issued_at = Some(now_duration);
    claims.expires_at = Some(now_duration + Duration::from_secs(3600));
    claims.invalid_before = Some(now_duration + Duration::from_secs(60));
    claims.custom = HashMap::new();

    let token = key_pair.sign(claims).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();

    let mut options_no_future = VerificationOptions::default();
    options_no_future.allowed_issuers = Some(HashSet::from([issuer.to_string()]));
    options_no_future.accept_future = false;
    options_no_future.time_tolerance = Some(Duration::from_secs(0));
    let result = public_key
        .verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options_no_future));
    assert!(result.is_err());

    let mut options_with_future = VerificationOptions::default();
    options_with_future.allowed_issuers = Some(HashSet::from([issuer.to_string()]));
    options_with_future.accept_future = true;
    let result = public_key
        .verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options_with_future));
    assert!(result.is_ok());
}
