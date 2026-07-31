use digua_jwt_ctx::{Principal, PrincipalError};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

fn make_token(payload: serde_json::Value) -> String {
    let header_b64 = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
    format!("{}.{}.sig", header_b64, payload_b64)
}

#[test]
fn parses_tid_claim_as_i64() {
    let token = make_token(serde_json::json!({"sub":"u1","tid":7}));
    let p = Principal::from_jwt_payload(&token).expect("parses");
    assert_eq!(p.tenant_id, 7);
}

#[test]
fn missing_tid_returns_error() {
    let token = make_token(serde_json::json!({"sub":"u1"}));
    let result = Principal::from_jwt_payload(&token);
    assert!(matches!(result, Err(PrincipalError::MissingTenantId)));
}

#[test]
fn non_integer_tid_returns_error() {
    let token = make_token(serde_json::json!({"sub":"u1","tid":"7"}));
    let result = Principal::from_jwt_payload(&token);
    assert!(matches!(result, Err(PrincipalError::MissingTenantId)));
}
