use crate::template::Template;
use crate::AppState;
use crate::RouteConfig;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use redis::aio::MultiplexedConnection;
use std::sync::Arc;

pub mod rate_limit;
pub mod tracing_mw;

pub fn is_public_route(routes: &[RouteConfig], path: &str) -> bool {
    routes
        .iter()
        .any(|r| r.public && upstream_matches(&r.upstream, path))
}

fn upstream_matches(upstream: &str, path: &str) -> bool {
    let tpl = match Template::parse(upstream) {
        Ok(t) => t,
        Err(_) => return false,
    };
    tpl.extract(path).is_ok()
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    if is_public_route(&state.route_table.load().routes, &path) {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.to_string());

    let token = match auth_header {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Missing Authorization header").into_response(),
    };

    let claims = match state.jwt_validator.validate(&token) {
        Ok(c) => c,
        Err(status) => return (status, "Invalid or expired token").into_response(),
    };

    let mut conn: MultiplexedConnection = state.redis_conn.clone();
    let blacklisted = crate::blacklist::is_blacklisted(&mut conn, &state.redis_prefix, &claims.jti)
        .await
        .unwrap_or(false);
    if blacklisted {
        return (StatusCode::UNAUTHORIZED, "Token revoked").into_response();
    }

    tracing::debug!(
        "authenticated user={} scope={} jti={}",
        claims.sub,
        claims.scope,
        claims.jti
    );

    let mut request = request;
    request.extensions_mut().insert(claims.clone());

    next.run(request).await
}
