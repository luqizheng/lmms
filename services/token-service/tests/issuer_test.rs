use jwt_simple::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

fn generate_test_keypair() -> (RS256KeyPair, String) {
    let key_pair = RS256KeyPair::generate(2048).unwrap();
    let public_key_pem = key_pair.public_key().to_pem().unwrap();
    (key_pair, public_key_pem)
}

#[test]
fn test_issue_access_token_success() {
    let issuer = "test-issuer";
    let (key_pair, _) = generate_test_keypair();

    let sub = "user-123";
    let client_id = "test-client";
    let permissions = vec!["read".to_string(), "write".to_string()];
    let ttl_sec: i64 = 3600;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some(sub.to_string());
    claims.jwt_id = Some(Uuid::new_v4().to_string());
    claims.issuer = Some(issuer.to_string());
    claims.expires_at = Some(now_duration + Duration::from_secs(ttl_sec as u64));

    let mut custom = HashMap::new();
    custom.insert("scope".to_string(), serde_json::Value::String("access".to_string()));
    custom.insert("client_id".to_string(), serde_json::Value::String(client_id.to_string()));
    custom.insert("permissions".to_string(), serde_json::Value::Array(
        permissions.iter().map(|p| serde_json::Value::String(p.clone())).collect(),
    ));
    claims.custom = custom;

    let token = key_pair.sign(claims).unwrap();
    assert!(!token.is_empty());
    assert!(token.contains('.'));

    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3);
}

#[test]
fn test_issue_refresh_token_success() {
    let issuer = "test-issuer";
    let (key_pair, _) = generate_test_keypair();

    let sub = "user-123";
    let ttl_sec: i64 = 86400;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let family = Uuid::new_v4().to_string();

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some(sub.to_string());
    claims.jwt_id = Some(Uuid::new_v4().to_string());
    claims.issuer = Some(issuer.to_string());
    claims.expires_at = Some(now_duration + Duration::from_secs(ttl_sec as u64));

    let mut custom = HashMap::new();
    custom.insert("scope".to_string(), serde_json::Value::String("refresh".to_string()));
    custom.insert("family".to_string(), serde_json::Value::String(family.clone()));
    claims.custom = custom;

    let token = key_pair.sign(claims).unwrap();
    assert!(!token.is_empty());

    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from([issuer.to_string()]));

    let verified = key_pair.public_key()
        .verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options))
        .unwrap();

    let verified_family = verified.custom.get("family").and_then(|v| v.as_str()).unwrap();
    assert_eq!(verified_family, family);
}

#[test]
fn test_issue_resource_token_success() {
    let issuer = "test-issuer";
    let (key_pair, _) = generate_test_keypair();

    let sub = "user-123";
    let resource_type = "document";
    let resource_id = "doc-456";
    let permission = "read";
    let ttl_sec: i64 = 1800;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some(sub.to_string());
    claims.jwt_id = Some(Uuid::new_v4().to_string());
    claims.issuer = Some(issuer.to_string());
    claims.expires_at = Some(now_duration + Duration::from_secs(ttl_sec as u64));

    let mut custom = HashMap::new();
    custom.insert("scope".to_string(), serde_json::Value::String("resource".to_string()));
    custom.insert("resource_type".to_string(), serde_json::Value::String(resource_type.to_string()));
    custom.insert("resource_id".to_string(), serde_json::Value::String(resource_id.to_string()));
    custom.insert("permission".to_string(), serde_json::Value::String(permission.to_string()));
    claims.custom = custom;

    let token = key_pair.sign(claims).unwrap();
    assert!(!token.is_empty());

    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from([issuer.to_string()]));

    let verified = key_pair.public_key()
        .verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options))
        .unwrap();

    assert_eq!(verified.custom.get("resource_type").and_then(|v| v.as_str()), Some(resource_type));
    assert_eq!(verified.custom.get("resource_id").and_then(|v| v.as_str()), Some(resource_id));
    assert_eq!(verified.custom.get("permission").and_then(|v| v.as_str()), Some(permission));
}

#[test]
fn test_access_token_verify_and_extract_claims() {
    let issuer = "test-issuer";
    let (key_pair, public_key_pem) = generate_test_keypair();

    let sub = "user-789";
    let client_id = "client-abc";
    let permissions = vec!["admin".to_string()];

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some(sub.to_string());
    claims.jwt_id = Some(Uuid::new_v4().to_string());
    claims.issuer = Some(issuer.to_string());
    claims.expires_at = Some(now_duration + Duration::from_secs(3600));

    let mut custom = HashMap::new();
    custom.insert("scope".to_string(), serde_json::Value::String("access".to_string()));
    custom.insert("client_id".to_string(), serde_json::Value::String(client_id.to_string()));
    custom.insert("permissions".to_string(), serde_json::Value::Array(
        permissions.iter().map(|p| serde_json::Value::String(p.clone())).collect(),
    ));
    claims.custom = custom;

    let token = key_pair.sign(claims).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from([issuer.to_string()]));

    let verified = public_key.verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options)).unwrap();

    assert_eq!(verified.subject, Some(sub.to_string()));
    assert!(!verified.jwt_id.unwrap().is_empty());
    assert_eq!(verified.issuer, Some(issuer.to_string()));
    assert_eq!(verified.custom.get("client_id").and_then(|v| v.as_str()), Some(client_id));
}

#[test]
fn test_token_expiration() {
    let issuer = "test-issuer";
    let (key_pair, public_key_pem) = generate_test_keypair();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some("user-123".to_string());
    claims.jwt_id = Some(Uuid::new_v4().to_string());
    claims.issuer = Some(issuer.to_string());
    claims.expires_at = Some(now_duration - Duration::from_secs(100));
    claims.invalid_before = Some(now_duration - Duration::from_secs(200));
    claims.custom = HashMap::new();

    let token = key_pair.sign(claims).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from([issuer.to_string()]));
    options.accept_future = false;
    options.time_tolerance = Some(Duration::from_secs(0));

    let result = public_key.verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options));
    assert!(result.is_err(), "Expected token verification to fail for expired token");
}

#[test]
fn test_refresh_token_family_tracking() {
    let issuer = "test-issuer";
    let (key_pair, public_key_pem) = generate_test_keypair();

    let family1 = Uuid::new_v4().to_string();
    let family2 = Uuid::new_v4().to_string();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let now_duration = Duration::from_secs(now);

    let mut claims1: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims1.subject = Some("user-123".to_string());
    claims1.jwt_id = Some(Uuid::new_v4().to_string());
    claims1.issuer = Some(issuer.to_string());
    claims1.expires_at = Some(now_duration + Duration::from_secs(86400));
    let mut custom1 = HashMap::new();
    custom1.insert("scope".to_string(), serde_json::Value::String("refresh".to_string()));
    custom1.insert("family".to_string(), serde_json::Value::String(family1.clone()));
    claims1.custom = custom1;

    let mut claims2 = claims1.clone();
    claims2.jwt_id = Some(Uuid::new_v4().to_string());
    let mut custom2 = HashMap::new();
    custom2.insert("scope".to_string(), serde_json::Value::String("refresh".to_string()));
    custom2.insert("family".to_string(), serde_json::Value::String(family2.clone()));
    claims2.custom = custom2;

    let token1 = key_pair.sign(claims1).unwrap();
    let token2 = key_pair.sign(claims2).unwrap();

    let public_key = RS256PublicKey::from_pem(&public_key_pem).unwrap();
    let mut options = VerificationOptions::default();
    options.allowed_issuers = Some(HashSet::from([issuer.to_string()]));

    let verified1 = public_key.verify_token::<HashMap<String, serde_json::Value>>(&token1, Some(options.clone())).unwrap();
    let verified2 = public_key.verify_token::<HashMap<String, serde_json::Value>>(&token2, Some(options)).unwrap();

    let fam1 = verified1.custom.get("family").and_then(|v| v.as_str()).unwrap();
    let fam2 = verified2.custom.get("family").and_then(|v| v.as_str()).unwrap();

    assert_eq!(fam1, family1);
    assert_eq!(fam2, family2);
    assert_ne!(fam1, fam2);
}
