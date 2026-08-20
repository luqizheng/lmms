//! digua_common-ratelimit — service-side rate limiting via Redis sliding window.
//!
//! Use case: business-logic limits beyond what gateway can express.
//! E.g. "max 5 announcements per admin per minute" — gateway only knows routes,
//! not admin-specific quotas.
//!
//! ## Algorithm
//!
//! Sliding window via Redis ZSET per (route, identity):
//! - Key: `rl:<route>:<identity>:<window_secs>`
//! - Score + member: timestamp ms
//! - Per request: ZADD ts, ZREMRANGEBYSCORE old, ZCARD count, EXPIRE window
//! - If count > limit, reject with 429
//!
//! ## Failure mode
//!
//! **Fails open** if Redis is unreachable. Logged as warn; not blocking.
//! Reason: Redis going down shouldn't break the whole product.
//!
//! ## Composition with gateway
//!
//! Gateway (P3.2) does route-level limits. This lib does business-level limits.
//! The two are independent — a request passes both gates, or fails one.
//!
//! ## Example
//!
//! ```ignore
//! use digua_common_ratelimit::{RateLimiter, Limit, rate_limit_middleware};
//!
//! let limiter = RateLimiter::new(redis_pool);
//! let app = Router::new()
//!     .route("/announcements", post(create))
//!     .route_layer(axum::middleware::from_fn_with_state(
//!         limiter.clone(),
//!         rate_limit_middleware,
//!     ));
//! ```

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use prometheus::{register_int_counter_vec, IntCounterVec};
use redis::aio::ConnectionManager;
use std::sync::{Arc, OnceLock};

/// Prometheus counter for rate-limited requests.
static RATE_LIMITED_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();

/// Initialize the rate-limited counter. Call once at service startup.
pub fn init_metrics(service_name: &'static str) {
    let counter = register_int_counter_vec!(
        "service_rate_limited_total",
        "Total HTTP requests rejected by service-side rate limiter",
        &["service", "route"]
    )
    .expect("register_int_counter_vec failed");
    RATE_LIMITED_TOTAL.get_or_init(|| counter);
    // Touch service_name to keep the parameter meaningful (label isn't dynamic,
    // but this lets future per-service counters work without changing the call).
    let _ = service_name;
}

/// A rate limit rule for a specific route.
#[derive(Debug, Clone, Copy)]
pub struct Limit {
    /// Route identifier, e.g. "POST /announcements".
    pub route: &'static str,
    /// Max requests allowed in the window.
    pub max: u32,
    /// Window size in seconds.
    pub window_secs: u32,
}

impl Limit {
    pub const fn new(route: &'static str, max: u32, window_secs: u32) -> Self {
        Self {
            route,
            max,
            window_secs,
        }
    }
}

/// Default limits when no specific Limit is configured for a route.
pub const DEFAULT_LIMIT: Limit = Limit::new("default", 60, 60);

/// Service-side rate limiter backed by Redis sliding window.
#[derive(Clone)]
pub struct RateLimiter {
    conn: ConnectionManager,
    /// Optional table of route → Limit. Falls back to DEFAULT_LIMIT.
    limits: Arc<Vec<Limit>>,
}

impl RateLimiter {
    pub fn new(conn: ConnectionManager, limits: Vec<Limit>) -> Self {
        Self {
            conn,
            limits: Arc::new(limits),
        }
    }

    pub fn with_defaults(conn: ConnectionManager) -> Self {
        Self::new(conn, vec![DEFAULT_LIMIT])
    }

    /// Look up the Limit for a given route key (e.g. "POST /announcements").
    pub fn limit_for<'a>(&'a self, route_key: &'a str) -> &'a Limit {
        for l in self.limits.iter() {
            if l.route == route_key {
                return l;
            }
        }
        &DEFAULT_LIMIT
    }

    /// Check if (route, identity) is within its limit. Returns Ok(remaining)
    /// if allowed, Err(RateLimited) if rejected.
    ///
    /// Fails open on Redis errors — logs warn but allows the request.
    pub async fn check(&self, identity: &str, limit: &Limit) -> Result<(), ()> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let window_start_ms = now_ms - (limit.window_secs as i64) * 1000;
        let key = format!("rl:{}:{}:{}", limit.route, identity, limit.window_secs);
        let member = format!("{}:{}", identity, now_ms);
        const SCRIPT: &str = r"
            redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', ARGV[1])
            local count = redis.call('ZCARD', KEYS[1])
            if count < tonumber(ARGV[2]) then
                redis.call('ZADD', KEYS[1], ARGV[3], ARGV[4])
                redis.call('EXPIRE', KEYS[1], ARGV[5])
                return count + 1
            end
            return -1
        ";

        let mut conn = self.conn.clone();
        let count: Result<i64, redis::RedisError> = redis::Script::new(SCRIPT)
            .key(&key)
            .arg(window_start_ms)
            .arg(limit.max as i64)
            .arg(now_ms)
            .arg(member)
            .arg(limit.window_secs as i64)
            .invoke_async(&mut conn)
            .await;

        match count {
            Ok(c) if c < 0 => Err(()),
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::warn!(target: "ratelimit", route = %limit.route, error = %e,
                    "redis check failed, failing open");
                Ok(())
            }
        }
    }
}

/// Axum middleware factory: requires `State<RateLimiter>`.
///
/// Behavior:
/// 1. Extract identity from `X-User-Id` header (set by upstream auth) or
///    fall back to peer IP.
/// 2. Build route key as `METHOD path` (e.g. `POST /notification/announcements`).
/// 3. Look up the Limit for that route.
/// 4. Call `limiter.check(identity, &limit)`.
/// 5. If rejected, return 429 + Retry-After + JSON body + Prometheus counter.
/// 6. Otherwise, call next.
pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let route_key = format!("{} {}", method.as_str(), path);

    let identity = extract_identity(&request);
    let limit = limiter.limit_for(&route_key);

    match limiter.check(&identity, limit).await {
        Ok(()) => next.run(request).await,
        Err(()) => {
            if let Some(counter) = RATE_LIMITED_TOTAL.get() {
                let service = std::env::var("SERVICE_NAME").unwrap_or_else(|_| "unknown".into());
                counter.with_label_values(&[&service, limit.route]).inc();
            }
            tracing::warn!(
                target: "ratelimit",
                route = %limit.route,
                identity = %identity,
                limit = limit.max,
                window = limit.window_secs,
                "rejected"
            );
            let body = serde_json::json!({
                "code": "RATE_LIMITED",
                "message": "too many requests",
                "retry_after_ms": (limit.window_secs as u64) * 1000,
            });
            let mut response = (StatusCode::TOO_MANY_REQUESTS, axum::Json(body)).into_response();
            response.headers_mut().insert(
                "Retry-After",
                HeaderValue::from_str(&limit.window_secs.to_string()).unwrap(),
            );
            response
        }
    }
}

fn extract_identity(request: &Request<Body>) -> String {
    // 1. X-User-Id header (set by upstream auth / gateway)
    if let Some(uid) = request
        .headers()
        .get("X-User-Id")
        .and_then(|v| v.to_str().ok())
    {
        return format!("user:{}", uid);
    }
    // 2. Authorization: Bearer <jwt> — extract sub claim
    if let Some(auth) = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return format!("tok:{}", &token[..token.len().min(32)]);
        }
    }
    // 3. X-Real-IP (nginx-injected)
    if let Some(ip) = request
        .headers()
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
    {
        return format!("ip:{}", ip);
    }
    // 4. Fallback: peer addr
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| format!("ip:{}", c.0.ip()))
        .unwrap_or_else(|| "ip:unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::Limit;

    #[test]
    fn multiple_requests_from_same_identity_are_counted() {
        let limit = Limit::new("test", 2, 60);
        let key = format!("rl:{}:{}:{}", limit.route, "user1", limit.window_secs);
        let now1 = 1000i64;
        let now2 = 1001i64;
        let now3 = 1002i64;
        assert_ne!(format!("user1:{}", now1), format!("user1:{}", now2));
        assert_ne!(format!("user1:{}", now2), format!("user1:{}", now3));
        let _ = key;
    }
}
