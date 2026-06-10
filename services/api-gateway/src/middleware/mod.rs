use crate::AppState;
use crate::RouteConfig;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use redis::aio::MultiplexedConnection;
use std::sync::Arc;

pub fn is_public_route(routes: &[RouteConfig], path: &str) -> bool {
    routes.iter().any(|r| {
        r.public && match_route_pattern(path, &r.match_pattern)
    })
}

fn match_route_pattern(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        let api_prefix = format!("/api{}", prefix);
        path == api_prefix || path.starts_with(&format!("{}/", api_prefix))
    } else {
        path == format!("/api{}", pattern)
    }
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
    inject_user_headers(request.headers_mut(), &claims);

    next.run(request).await
}

/// 把 JWT claims 里的 user 字段写入 X-User-* 冗余头，便于下游审计与字段级 fallback。
/// 字段从 claims.custom 读，user-service 决定 schema。
pub fn inject_user_headers(headers: &mut HeaderMap, claims: &crate::jwt::JwtClaims) {
    if let Ok(v) = HeaderValue::from_str(&claims.sub) {
        headers.insert(HeaderName::from_static("x-user-id"), v);
    }
    if let Some(v) = claims.custom.get("dept_id").and_then(|v| v.as_u64()) {
        if let Ok(hv) = HeaderValue::from_str(&v.to_string()) {
            headers.insert(HeaderName::from_static("x-user-dept"), hv);
        }
    }
    if let Some(b) = claims.custom.get("is_dept_leader").and_then(|v| v.as_bool()) {
        headers.insert(
            HeaderName::from_static("x-user-dept-leader"),
            HeaderValue::from_static(if b { "true" } else { "false" }),
        );
    }
    if let Some(arr) = claims.custom.get("roles").and_then(|v| v.as_array()) {
        let joined: Vec<String> = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        if let Ok(hv) = HeaderValue::from_str(&joined.join(",")) {
            headers.insert(HeaderName::from_static("x-user-roles"), hv);
        }
    }
    if let Some(arr) = claims.custom.get("permissions").and_then(|v| v.as_array()) {
        let joined: Vec<String> = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        if let Ok(hv) = HeaderValue::from_str(&joined.join(",")) {
            headers.insert(HeaderName::from_static("x-user-perms"), hv);
        }
    }
    if let Some(s) = claims.custom.get("client_id").and_then(|v| v.as_str()) {
        if let Ok(hv) = HeaderValue::from_str(s) {
            headers.insert(HeaderName::from_static("x-user-client"), hv);
        }
    }
}
