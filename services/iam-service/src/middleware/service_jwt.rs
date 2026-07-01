//! Service-token gate for /iam/internal/* endpoints.
//!
//! Accepts only JWTs whose `aud` claim contains `"service"` and which carry a
//! `tid` claim. User tokens (admin / standard) are rejected on these routes.
//! Token signature is NOT verified (the api-gateway has already done so); we
//! decode the payload only.

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct JwtPayload {
    #[serde(default)]
    aud: Value,        // string or array
    #[serde(default)]
    tid: Value,
}

pub async fn require_service_jwt(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let auth = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(unauthorized)?;
    let token = auth
        .strip_prefix("Bearer ")
        .ok_or_else(unauthorized)?;
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(unauthorized());
    }
    let payload_bytes = base64_url_decode(parts[1])
        .map_err(|_| unauthorized())?;
    let p: JwtPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| unauthorized())?;
    if !claim_has(&p.aud, "service") {
        return Err(unauthorized());
    }
    if !p.tid.is_i64() && !p.tid.is_u64() {
        return Err(unauthorized());
    }
    Ok(next.run(request).await)
}

fn claim_has(v: &Value, needle: &str) -> bool {
    match v {
        Value::String(s) => s == needle,
        Value::Array(arr) => arr.iter().any(|x| x.as_str() == Some(needle)),
        _ => false,
    }
}

fn base64_url_decode(s: &str) -> Result<Vec<u8>, ()> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.decode(s).map_err(|_| ())
}

fn unauthorized() -> Response {
    let body = serde_json::json!({
        "code": "IAM_TOKEN_3004",
        "message": "service token required",
    });
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_token(payload: &str) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\",\"typ\":\"JWT\"}");
        let body = URL_SAFE_NO_PAD.encode(payload.as_bytes());
        format!("{header}.{body}.signature")
    }

    #[test]
    fn admin_token_is_rejected() {
        let tok = make_token(r#"{"sub":"u-admin","tid":1,"roles":["admin"]}"#);
        assert_eq!(audience_contains(&tok, "service"), false);
        assert_eq!(tid_present(&tok), true);
    }

    #[test]
    fn service_token_array_aud_is_accepted() {
        let tok = make_token(r#"{"aud":["service","internal"],"tid":1}"#);
        assert_eq!(audience_contains(&tok, "service"), true);
    }

    #[test]
    fn service_token_string_aud_is_accepted() {
        let tok = make_token(r#"{"aud":"service","tid":1}"#);
        assert_eq!(audience_contains(&tok, "service"), true);
    }

    #[test]
    fn missing_tid_is_rejected() {
        let tok = make_token(r#"{"aud":"service"}"#);
        assert_eq!(tid_present(&tok), false);
    }

    fn audience_contains(tok: &str, needle: &str) -> bool {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let parts: Vec<&str> = tok.split('.').collect();
        let bytes = URL_SAFE_NO_PAD.decode(parts[1]).unwrap();
        let p: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        claim_has(&p.get("aud").cloned().unwrap_or(Value::Null), needle)
    }
    fn tid_present(tok: &str) -> bool {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let parts: Vec<&str> = tok.split('.').collect();
        let bytes = URL_SAFE_NO_PAD.decode(parts[1]).unwrap();
        let p: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let v = &p.get("tid").cloned().unwrap_or(Value::Null);
        v.is_i64() || v.is_u64()
    }
}