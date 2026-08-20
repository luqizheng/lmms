// 真实 TokenIssuer API 单元测试，不依赖外部服务
use jwt_simple::prelude::*;
use std::collections::HashMap;
use token_service::TokenIssuer;

const TEST_ISSUER: &str = "test-issuer";

fn build_issuer() -> (TokenIssuer, String) {
    let key_pair = RS256KeyPair::generate(2048).unwrap();
    let private_pem = key_pair.to_pem().unwrap();
    let public_pem = key_pair.public_key().to_pem().unwrap();
    let issuer = TokenIssuer::new(&private_pem, TEST_ISSUER).expect("create issuer");
    (issuer, public_pem)
}

fn verify(token: &str, public_pem: &str) -> JWTClaims<HashMap<String, serde_json::Value>> {
    let public_key = RS256PublicKey::from_pem(public_pem).unwrap();
    let options = VerificationOptions {
        allowed_issuers: Some(HashSet::from([TEST_ISSUER.to_string()])),
        ..Default::default()
    };
    public_key
        .verify_token::<HashMap<String, serde_json::Value>>(token, Some(options))
        .expect("verify token")
}

#[test]
fn token_issuer_new_invalid_pem_fails() {
    let res = TokenIssuer::new("not-a-pem", TEST_ISSUER);
    assert!(res.is_err());
}

#[test]
fn issue_access_token_contains_scope_and_client() {
    let (issuer, public_pem) = build_issuer();
    let extra = serde_json::json!({
        "permissions": ["read", "write"],
        "user_id": "u-1",
    });
    let token = issuer
        .issue_access_token("user-123", "client-abc", extra, 3600)
        .unwrap();
    let claims = verify(&token, &public_pem);

    assert_eq!(claims.subject.as_deref(), Some("user-123"));
    assert_eq!(claims.issuer.as_deref(), Some(TEST_ISSUER));
    assert!(!claims.jwt_id.unwrap().is_empty());
    assert_eq!(
        claims.custom.get("scope").and_then(|v| v.as_str()),
        Some("access")
    );
    assert_eq!(
        claims.custom.get("client_id").and_then(|v| v.as_str()),
        Some("client-abc")
    );
    // extra_claims 必须合并进 custom
    assert_eq!(
        claims.custom.get("user_id").and_then(|v| v.as_str()),
        Some("u-1")
    );
    let perms = claims
        .custom
        .get("permissions")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(perms.len(), 2);
}

#[test]
fn issue_refresh_token_returns_unique_family() {
    let (issuer, public_pem) = build_issuer();
    let (token1, fam1) = issuer.issue_refresh_token("user-1", 86400).unwrap();
    let (token2, fam2) = issuer.issue_refresh_token("user-1", 86400).unwrap();
    assert_ne!(fam1, fam2, "每次签发 family 必须独立");

    let c1 = verify(&token1, &public_pem);
    let c2 = verify(&token2, &public_pem);
    assert_eq!(
        c1.custom.get("scope").and_then(|v| v.as_str()),
        Some("refresh")
    );
    assert_eq!(
        c1.custom.get("family").and_then(|v| v.as_str()),
        Some(fam1.as_str())
    );
    assert_eq!(
        c2.custom.get("family").and_then(|v| v.as_str()),
        Some(fam2.as_str())
    );
    assert_ne!(c1.jwt_id, c2.jwt_id, "jti 必须唯一");
}

#[test]
fn issue_user_jwt_sets_audience_and_filters_reserved() {
    let (issuer, public_pem) = build_issuer();
    // 故意塞一些 reserved 字段，应被过滤
    let extra = serde_json::json!({
        "iss": "evil",
        "sub": "evil",
        "exp": 0,
        "tenant": "acme",
        "roles": ["admin"],
    });
    let token = issuer
        .issue_user_jwt("user-77", "USR077", extra, 600)
        .unwrap();
    let claims = verify(&token, &public_pem);

    assert_eq!(claims.subject.as_deref(), Some("user-77"));
    assert_eq!(claims.issuer.as_deref(), Some(TEST_ISSUER));
    // audience
    match claims.audiences {
        Some(Audiences::AsString(ref s)) => assert_eq!(s, "coder-micro-services"),
        _ => panic!("audience 必须为 coder-micro-services"),
    }
    assert_eq!(
        claims.custom.get("scope").and_then(|v| v.as_str()),
        Some("access")
    );
    assert_eq!(
        claims.custom.get("client_id").and_then(|v| v.as_str()),
        Some("credential-login-service")
    );
    assert_eq!(
        claims.custom.get("user_code").and_then(|v| v.as_str()),
        Some("USR077")
    );
    assert_eq!(
        claims.custom.get("tenant").and_then(|v| v.as_str()),
        Some("acme")
    );
    // reserved 字段不应进 custom
    assert!(!claims.custom.contains_key("iss"));
    assert!(!claims.custom.contains_key("sub"));
    assert!(!claims.custom.contains_key("exp"));
}

#[test]
fn issue_resource_token_carries_resource_fields() {
    let (issuer, public_pem) = build_issuer();
    let token = issuer
        .issue_resource_token("user-9", "document", "doc-456", "read", 1800)
        .unwrap();
    let claims = verify(&token, &public_pem);

    assert_eq!(claims.subject.as_deref(), Some("user-9"));
    assert_eq!(
        claims.custom.get("scope").and_then(|v| v.as_str()),
        Some("resource")
    );
    assert_eq!(
        claims.custom.get("resource_type").and_then(|v| v.as_str()),
        Some("document")
    );
    assert_eq!(
        claims.custom.get("resource_id").and_then(|v| v.as_str()),
        Some("doc-456")
    );
    assert_eq!(
        claims.custom.get("permission").and_then(|v| v.as_str()),
        Some("read")
    );
}

#[test]
fn wrong_issuer_fails_verification() {
    let (issuer, public_pem) = build_issuer();
    let token = issuer
        .issue_access_token("u", "c", serde_json::json!({}), 3600)
        .unwrap();
    let public_key = RS256PublicKey::from_pem(&public_pem).unwrap();
    let options = VerificationOptions {
        allowed_issuers: Some(HashSet::from(["other-issuer".to_string()])),
        ..Default::default()
    };
    let res = public_key.verify_token::<HashMap<String, serde_json::Value>>(&token, Some(options));
    assert!(res.is_err(), "issuer 不匹配必须验签失败");
}
