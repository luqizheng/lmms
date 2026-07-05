use axum::http::HeaderMap;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use jwt_ctx::{Principal, PrincipalError};
use std::collections::HashSet;

fn create_test_jwt(payload: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode("{\"alg\":\"HS256\",\"typ\":\"JWT\"}");
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload);
    format!("{}.{}.signature", header, payload_b64)
}

fn jwt_payload(payload: serde_json::Value) -> String {
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
    let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
    format!("{}.{}.signature", header, body)
}

#[test]
fn parses_jwt_payload() {
    let payload = r#"{"sub":"user-uuid-1","user_code":"aB3xY9zQ","tid":1,"dept_id":7,"roles":["admin","sales"],"permissions":["user.read","user.write"]}"#;
    let token = create_test_jwt(payload);
    let mut h = HeaderMap::new();
    h.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
    
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.user_id, "user-uuid-1");
    assert_eq!(p.user_code, "aB3xY9zQ");
    assert_eq!(p.dept_id, Some(7));
    assert_eq!(p.roles, vec!["admin".to_string(), "sales".to_string()]);
    assert_eq!(
        p.permissions,
        HashSet::from(["user.read".into(), "user.write".into()])
    );
}

#[test]
fn missing_authorization_is_error() {
    let h = HeaderMap::new();
    assert!(Principal::from_headers(&h).is_err());
}

#[test]
fn invalid_authorization_format_is_error() {
    let mut h = HeaderMap::new();
    h.insert("Authorization", "InvalidToken".parse().unwrap());
    assert!(Principal::from_headers(&h).is_err());
}

#[test]
fn optional_fields_default() {
    let payload = r#"{"sub":"u","tid":1}"#;
    let token = create_test_jwt(payload);
    let mut h = HeaderMap::new();
    h.insert("Authorization", format!("Bearer {}", token).parse().unwrap());

    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.user_id, "u");
    assert_eq!(p.user_code, "");
    assert_eq!(p.dept_id, None);
    assert!(p.roles.is_empty());
    assert!(p.permissions.is_empty());
}

#[test]
fn has_role_matches_existing() {
    let payload = r#"{"sub":"u","tid":1,"roles":["admin","ops"]}"#;
    let token = create_test_jwt(payload);
    let mut h = HeaderMap::new();
    h.insert("Authorization", format!("Bearer {}", token).parse().unwrap());

    let p = Principal::from_headers(&h).unwrap();
    assert!(p.has_role("admin"));
    assert!(p.has_role("ops"));
    assert!(!p.has_role("guest"));
}

#[test]
fn has_role_empty_string_is_filtered() {
    let payload = r#"{"sub":"u","tid":1,"roles":["admin","","ops"]}"#;
    let token = create_test_jwt(payload);
    let mut h = HeaderMap::new();
    h.insert("Authorization", format!("Bearer {}", token).parse().unwrap());

    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.roles, vec!["admin".to_string(), "ops".to_string()]);
    assert!(!p.has_role(""));
}

#[test]
fn has_permission_matches_existing() {
    let payload = r#"{"sub":"u","tid":1,"permissions":["user.read","user.write"]}"#;
    let token = create_test_jwt(payload);
    let mut h = HeaderMap::new();
    h.insert("Authorization", format!("Bearer {}", token).parse().unwrap());

    let p = Principal::from_headers(&h).unwrap();
    assert!(p.has_permission("user.read"));
    assert!(p.has_permission("user.write"));
    assert!(!p.has_permission("user.delete"));
}

#[test]
fn has_permission_empty_string_is_filtered() {
    let payload = r#"{"sub":"u","tid":1,"permissions":["","user.read",""]}"#;
    let token = create_test_jwt(payload);
    let mut h = HeaderMap::new();
    h.insert("Authorization", format!("Bearer {}", token).parse().unwrap());

    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.permissions.len(), 1);
    assert!(p.has_permission("user.read"));
    assert!(!p.has_permission(""));
}

#[test]
fn invalid_jwt_format_is_error() {
    let mut h = HeaderMap::new();
    h.insert("Authorization", "Bearer invalid.token".parse().unwrap());
    assert!(Principal::from_headers(&h).is_err());
}

#[test]
fn invalid_base64_payload_is_error() {
    let mut h = HeaderMap::new();
    h.insert("Authorization", "Bearer header.invalid$$base64.signature".parse().unwrap());
    assert!(Principal::from_headers(&h).is_err());
}

#[test]
fn missing_sub_claim_is_error() {
    let payload = r#"{"user_code":"test"}"#;
    let token = create_test_jwt(payload);
    let mut h = HeaderMap::new();
    h.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
    assert!(Principal::from_headers(&h).is_err());
}

#[test]
fn missing_tid_returns_error() {
    let token = jwt_payload(serde_json::json!({
        "sub": "u_1",
        "user_code": "alice",
        "roles": ["tenant_admin"],
        "permissions": ["iam.user.read"]
    }));
    let result = Principal::from_jwt_payload(&token);
    assert!(matches!(result, Err(PrincipalError::MissingTenantId)));
}

#[test]
fn cross_tenant_parsed_from_ct_claim() {
    let token = jwt_payload(serde_json::json!({
        "sub": "u_1",
        "user_code": "admin",
        "tid": 1,
        "ct": true,
        "roles": ["platform_admin"],
        "permissions": ["iam.user.read"]
    }));
    let p = Principal::from_jwt_payload(&token).unwrap();
    assert_eq!(p.tenant_id, 1);
    assert!(p.cross_tenant);
}

#[test]
fn cross_tenant_defaults_false_when_missing() {
    let token = jwt_payload(serde_json::json!({
        "sub": "u_1",
        "tid": 7,
        "roles": ["tenant_admin"],
        "permissions": []
    }));
    let p = Principal::from_jwt_payload(&token).unwrap();
    assert!(!p.cross_tenant);
}