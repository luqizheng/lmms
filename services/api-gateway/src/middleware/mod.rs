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

/// 从 URI query string 提 `token=<jwt>`，用于 EventSource / WebSocket
/// 等无法设自定义 header 的客户端。Header 优先，本函数仅作 fallback。
fn extract_token_from_query(uri: &axum::http::Uri) -> Option<String> {
    uri.query().and_then(|q| {
        q.split('&').find_map(|kv| {
            let mut parts = kv.splitn(2, '=');
            let k = parts.next()?;
            let v = parts.next()?;
            if k == "token" && !v.is_empty() {
                url_decode(v)
            } else {
                None
            }
        })
    })
}

fn url_decode(s: &str) -> Option<String> {
    // 极简 percent-decode：处理 %XX 与 +
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
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

    let query_token = extract_token_from_query(request.uri());
    let token = match (auth_header, query_token) {
        (Some(h), _) => Some(h),
        (None, Some(q)) => Some(q),
        (None, None) => None,
    };

    let token = match token {
        Some(t) => t,
        None => return (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization header or ?token= query",
        )
            .into_response(),
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

    if crate::perm_epoch::check_revoked(&mut conn, &state.redis_prefix, &claims.sub, claims.iat).await
    {
        return (
            StatusCode::UNAUTHORIZED,
            "permissions changed, re-authenticate",
        )
            .into_response();
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
