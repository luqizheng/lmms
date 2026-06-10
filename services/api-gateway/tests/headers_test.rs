use api_gateway::jwt::JwtClaims;
use api_gateway::middleware::inject_user_headers;
use axum::http::HeaderMap;
use std::collections::HashMap;

fn mk_claims() -> JwtClaims {
    let mut custom = HashMap::new();
    custom.insert("dept_id".into(), serde_json::json!(42));
    custom.insert("is_dept_leader".into(), serde_json::json!(true));
    custom.insert("roles".into(), serde_json::json!(["sales", "dept_leader"]));
    custom.insert("permissions".into(), serde_json::json!(["contract.read", "contract.write"]));
    custom.insert("client_id".into(), serde_json::json!("web-spa"));

    JwtClaims {
        sub: "1003".into(),
        jti: "j".into(),
        iss: "i".into(),
        iat: 0,
        exp: 0,
        scope: "access".into(),
        client_id: "ignored".into(),
        permissions: vec!["legacy".into()],
        custom,
    }
}

#[test]
fn inject_user_headers_reads_custom_claims() {
    let mut headers = HeaderMap::new();
    let claims = mk_claims();
    inject_user_headers(&mut headers, &claims);

    assert_eq!(headers.get("x-user-id").unwrap(), "1003");
    assert_eq!(headers.get("x-user-dept").unwrap(), "42");
    assert_eq!(headers.get("x-user-dept-leader").unwrap(), "true");
    assert_eq!(headers.get("x-user-roles").unwrap(), "sales,dept_leader");
    assert_eq!(headers.get("x-user-perms").unwrap(), "contract.read,contract.write");
    assert_eq!(headers.get("x-user-client").unwrap(), "web-spa");
}

#[test]
fn inject_user_headers_minimal_claims() {
    let mut headers = HeaderMap::new();
    let claims = JwtClaims {
        sub: "1".into(),
        ..Default::default()
    };
    inject_user_headers(&mut headers, &claims);
    assert_eq!(headers.get("x-user-id").unwrap(), "1");
    assert!(headers.get("x-user-dept").is_none());
    assert!(headers.get("x-user-roles").is_none());
}
