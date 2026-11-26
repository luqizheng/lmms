use crate::template::Template;
use crate::AppState;
use crate::RouteConfig;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use digua_common_jwt;
use redis::aio::MultiplexedConnection;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// 2026-11-05 review (P0-3): reject requests whose path contains dot-segments
/// (`.` / `..`, including percent-encoded variants) or backslashes, BEFORE the
/// template matcher authorizes the un-normalized path.
///
/// Root cause chain: the gateway matches and authorizes the raw wire path,
/// but reqwest's `Url::parse` (WHATWG) removes `..` segments from the
/// upstream URL — so `/v1/iam/users/../../internal/users/{id}/claims` is
/// authorized as `/v1/iam/users/{rest}` (protected) and then forwarded as
/// `/internal/users/{id}/claims` (unauthenticated internal endpoint). The
/// same path-confusion applies to `\` on WHATWG parsers.
pub async fn path_sanitize_middleware(request: Request<Body>, next: Next) -> Response {
    if path_has_traversal(request.uri().path()) {
        return (StatusCode::NOT_FOUND, "path rejected").into_response();
    }
    next.run(request).await
}

/// Percent-decode then reject dot-segments and backslashes. Decoding first
/// closes the `%2e%2e` / `%2F` bypasses; `%2F` decoded to `/` is caught by
/// the segment split itself.
fn path_has_traversal(raw_path: &str) -> bool {
    let decoded = percent_decode(raw_path);
    decoded
        .split('/')
        .any(|seg| seg == "." || seg == ".." || seg.contains('\\'))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push(((h * 16) + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod path_sanitize_tests {
    use super::path_has_traversal;

    #[test]
    fn rejects_dot_dot_segments() {
        assert!(path_has_traversal(
            "/v1/iam/users/../../internal/users/1/claims"
        ));
    }

    #[test]
    fn rejects_percent_encoded_dot_dot() {
        assert!(path_has_traversal("/v1/iam/users/%2e%2e/internal"));
        assert!(path_has_traversal("/v1/iam/users/%2E%2E/internal"));
    }

    #[test]
    fn rejects_encoded_slash_with_dot_dot() {
        assert!(path_has_traversal("/v1/iam/users%2F..%2Finternal"));
    }

    #[test]
    fn rejects_backslash_segments() {
        assert!(path_has_traversal("/v1/file/..\\..\\etc/passwd"));
    }

    #[test]
    fn allows_normal_paths() {
        assert!(!path_has_traversal("/v1/iam/users/me"));
        assert!(!path_has_traversal("/v1/file/upload"));
        assert!(!path_has_traversal("/v1/config/item/config-1"));
        assert!(!path_has_traversal("/health"));
    }
}

#[derive(Debug, Clone, Default)]
pub struct JwtClaims {
    pub sub: String,
    pub jti: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
    pub scope: String,
    pub client_id: String,
    pub permissions: Vec<String>,
    pub roles: Vec<String>,
    pub custom: HashMap<String, Value>,
}

fn extract_claims_from_value(claims: &Value) -> JwtClaims {
    let sub = claims["sub"].as_str().unwrap_or("").to_string();
    let jti = claims["jti"].as_str().unwrap_or("").to_string();
    let iss = claims["iss"].as_str().unwrap_or("").to_string();
    let iat = claims["iat"].as_i64().unwrap_or(0);
    let exp = claims["exp"].as_i64().unwrap_or(0);
    let scope = claims["scope"].as_str().unwrap_or("").to_string();
    let client_id = claims["client_id"].as_str().unwrap_or("").to_string();
    let permissions: Vec<String> = claims["permissions"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let roles: Vec<String> = claims["roles"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let custom: HashMap<String, Value> = claims
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "sub"
                            | "jti"
                            | "iss"
                            | "iat"
                            | "exp"
                            | "scope"
                            | "client_id"
                            | "permissions"
                            | "roles"
                    )
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    JwtClaims {
        sub,
        jti,
        iss,
        iat,
        exp,
        scope,
        client_id,
        permissions,
        roles,
        custom,
    }
}

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
            let (k, v) = kv.split_once('=')?;
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
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                "Missing Authorization header or ?token= query",
            )
                .into_response()
        }
    };

    let claims_value = match digua_common_jwt::verify_token(&token, &state.jwks_cache, None).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "jwt verification failed");
            return (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response();
        }
    };
    let claims = extract_claims_from_value(&claims_value);

    // Phase 8.3: fail-fast on missing tid claim
    if !claims.custom.contains_key("tid") {
        return (StatusCode::UNAUTHORIZED, "missing tenant claim").into_response();
    }

    // 撤销名单检查走 TokenBlacklistService（WP-TEST-01-R1 · DEC-02，与
    // Redis 存储解耦；iam-service logout 写入侧共用同一 key 契约）。
    // 错误策略保持 2026-07-14 1910c405 既有决策：fail-open + warn，
    // 避免 Redis 故障阻断全部请求；service 层本身如实传播错误、不吞错。
    let blacklisted = match state.blacklist.is_revoked(&claims.jti).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "blacklist check error, failing open");
            false
        }
    };
    if blacklisted {
        return (StatusCode::UNAUTHORIZED, "Token revoked").into_response();
    }

    let mut conn: MultiplexedConnection = state.redis_conn.clone();

    let sub_parsed: u64 = claims.sub.parse().unwrap_or(0u64);
    let (cached_epoch, hit) = state.perm_epoch_cache.get(sub_parsed);
    let epoch = if hit {
        cached_epoch
    } else {
        let epoch = digua_common_event::perm_epoch::get_perm_epoch(
            &mut conn,
            &state.redis_prefix,
            &claims.sub,
        )
        .await
        .ok()
        .flatten();
        state.perm_epoch_cache.set(sub_parsed, epoch);
        epoch
    };
    if crate::perm_epoch::is_revoked(epoch, claims.iat) {
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
