use axum::{
    extract::{FromRequestParts, TypedHeader},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use jwt_ctx::{Principal, TokenExtractor};
use tracing::info;

pub async fn auth_middleware<B>(
    TokenExtractor(principal): TokenExtractor,
    mut req: axum::http::Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    req.extensions_mut().insert(principal.clone());
    Ok(next.run(req).await)
}

pub async fn require_permission<B>(
    permission: &'static str,
    principal: Principal,
    req: axum::http::Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if !principal.has_permission(permission) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
}

pub async fn require_role<B>(
    role: &'static str,
    principal: Principal,
    req: axum::http::Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if !principal.has_role(role) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
}
