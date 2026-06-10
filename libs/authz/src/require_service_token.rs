use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use futures::future::BoxFuture;
use jwt_simple::algorithms::RS256PublicKey;
use jwt_simple::prelude::*;
use std::collections::HashMap;

pub fn require_service_token(
    expected_aud: &'static str,
    public_key_pem: String,
) -> impl Fn(Request, Next) -> BoxFuture<'static, Result<Response, StatusCode>>
     + Clone
     + Send
     + Sync
     + 'static {
    let pk = RS256PublicKey::from_pem(&public_key_pem).expect("invalid public key pem");
    move |req: Request, next: Next| {
        let pk = pk.clone();
        let expected_aud = expected_aud;
        Box::pin(async move {
            let token = match req
                .headers()
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
            {
                Some(t) => t.to_string(),
                None => return Err(StatusCode::UNAUTHORIZED),
            };
            let claims: JWTClaims<HashMap<String, serde_json::Value>> = match pk.verify_token(&token, None) {
                Ok(c) => c,
                Err(_) => return Err(StatusCode::UNAUTHORIZED),
            };
            let token_type = claims.custom.get("token_type").and_then(|v| v.as_str());
            if token_type != Some("service") {
                return Err(StatusCode::UNAUTHORIZED);
            }
            let aud_ok = match &claims.audiences {
                Some(Audiences::AsString(s)) => s == expected_aud,
                Some(Audiences::AsSet(set)) => set.iter().any(|a| a == expected_aud),
                None => false,
            };
            if !aud_ok {
                return Err(StatusCode::FORBIDDEN);
            }
            Ok(next.run(req).await)
        })
    }
}
