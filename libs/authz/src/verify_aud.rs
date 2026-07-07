//! Phase 5-B: middleware that gates a route by requiring a valid JWT
//! with `aud == expected_aud`.
//!
//! Use `from_fn_with_state` to inject a state type that implements
//! `jwt_ctx::PrincipalStateProvider`.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jwt_ctx::{Principal, PrincipalStateProvider};

/// Middleware factory. Returns a closure suitable for
/// `axum::middleware::from_fn_with_state`.
///
/// Behaviour: extracts the `Authorization: Bearer <jwt>` header, runs
/// `Principal::from_token(token, &state.jwks(), Some(expected_aud))`.
/// If verification fails (signature / aud mismatch / expired), returns
/// `401 Unauthorized`. Otherwise, the request continues with the verified
/// principal (handlers may re-extract via `Principal` extractor at no
/// verification cost).
pub fn verify_aud_principal_middleware<S>(
    expected_aud: &'static str,
) -> impl Fn(State<S>, Request, Next) -> futures::future::BoxFuture<'static, Response>
       + Clone
       + Send
       + Sync
       + 'static
where
    S: PrincipalStateProvider + Send + Sync + 'static,
{
    move |State(state), req, next| {
        let fut = async move {
            let (mut parts, body) = req.into_parts();
            let token = match parts
                .headers
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
            {
                Some(t) => t.to_string(),
                None => return reject(StatusCode::UNAUTHORIZED),
            };
            let cache = state.jwks();
            match Principal::from_token(&token, &cache, Some(expected_aud)).await {
                Ok(_p) => {
                    let req = Request::from_parts(parts, body);
                    next.run(req).await
                }
                Err(_) => reject(StatusCode::UNAUTHORIZED),
            }
        };
        Box::pin(fut)
    }
}

fn reject(status: StatusCode) -> Response {
    use axum::response::IntoResponse;
    (status, "unauthorized").into_response()
}
