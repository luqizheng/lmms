use token_service::openapi::ApiDoc;
use utoipa::OpenApi;

#[test]
fn openapi_contains_core_paths() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_value(&doc).expect("serialize openapi");
    let paths = json
        .get("paths")
        .and_then(|v| v.as_object())
        .expect("paths object");

    assert!(paths.contains_key("/tokens/access"));
    assert!(paths.contains_key("/tokens/refresh"));
    assert!(paths.contains_key("/tokens/revoke"));
    assert!(paths.contains_key("/tokens/user-jwt"));
    assert!(paths.contains_key("/admin/oauth-clients"));
}

#[test]
fn openapi_has_bearer_security_scheme() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_value(&doc).expect("serialize openapi");
    let scheme = json
        .pointer("/components/securitySchemes/bearer_auth")
        .expect("bearer_auth scheme present");
    assert_eq!(scheme.get("type").and_then(|v| v.as_str()), Some("http"));
    assert_eq!(
        scheme.get("scheme").and_then(|v| v.as_str()),
        Some("bearer")
    );
    assert_eq!(
        scheme.get("bearerFormat").and_then(|v| v.as_str()),
        Some("JWT")
    );
}
