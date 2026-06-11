use axum::{extract::Request, middleware::Next, response::Response};

/// Internal endpoint auth: require service token signed by token-service.
///
/// Currently a no-op pass-through. Once `Config.auth` is extended with
/// `service_token_public_key_pem` (read from `services/user-service/config/config.yaml`),
/// wire this to `authz::require_service_token(expected_aud, pk)(req, next).await`.
pub async fn require_internal_service_token(
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    Ok(next.run(req).await)
}
