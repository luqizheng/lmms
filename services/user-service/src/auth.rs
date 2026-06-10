use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

/// Internal endpoint auth: require service token signed by token-service.
/// Delegates to `authz::require_service_token`.
pub async fn require_internal_service_token(
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    authz::require_service_token(req, next).await
}
