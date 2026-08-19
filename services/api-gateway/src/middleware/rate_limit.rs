use crate::AppState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use prometheus::IntCounterVec;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub static RATE_LIMITED_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();

pub fn init_metrics() {
    RATE_LIMITED_TOTAL
        .get_or_init(|| {
            prometheus::register_int_counter_vec!(
                "gateway_rate_limited_total",
                "Total HTTP requests rejected by rate limiter",
                &["route"]
            )
            .unwrap()
        });
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitRule {
    pub requests_per_second: u32,
    pub burst: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_rule")]
    pub defaults: RateLimitRule,
    #[serde(default)]
    pub routes: HashMap<String, RateLimitRule>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            defaults: default_rule(),
            routes: HashMap::new(),
        }
    }
}

fn default_rule() -> RateLimitRule {
    RateLimitRule {
        requests_per_second: 100,
        burst: 200,
    }
}

impl RateLimitConfig {
    pub fn rule_for(&self, path: &str) -> &RateLimitRule {
        for (pattern, rule) in &self.routes {
            if path_matches(pattern, path) {
                return rule;
            }
        }
        &self.defaults
    }
}

fn path_matches(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        path.starts_with(prefix)
    } else {
        pattern == path
    }
}

#[derive(Clone)]
pub struct RouteLimiterSet {
    limiter: digua_common_ratelimit::RateLimiter,
    // ponytail: Box::leak at startup for static route strings — < 20 routes, acceptable.
    limits: Arc<HashMap<String, digua_common_ratelimit::Limit>>,
    default: digua_common_ratelimit::Limit,
}

impl RouteLimiterSet {
    pub fn new(
        conn: redis::aio::ConnectionManager,
        cfg: &RateLimitConfig,
    ) -> Self {
        use digua_common_ratelimit::Limit;
        let mut limits: HashMap<String, Limit> = HashMap::new();
        for (pattern, rule) in &cfg.routes {
            let route: &'static str = Box::leak(
                format!("gateway:{}", pattern).into_boxed_str()
            );
            limits.insert(pattern.clone(), Limit {
                route,
                max: rule.burst,
                window_secs: 1,
            });
        }
        let default_route: &'static str = Box::leak("gateway:default".to_string().into_boxed_str());
        Self {
            limiter: digua_common_ratelimit::RateLimiter::new(conn, vec![]),
            limits: Arc::new(limits),
            default: Limit {
                route: default_route,
                max: cfg.defaults.burst,
                window_secs: 1,
            },
        }
    }

    pub async fn check(&self, path: &str, identity: &str) -> Result<(), ()> {
        let limit = self.limits
            .iter()
            .find(|(p, _)| path_matches(p, path))
            .map(|(_, l)| l)
            .unwrap_or(&self.default);
        self.limiter.check(identity, limit).await
    }
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let identity_key = extract_identity_key(&request);

    if state.route_limiters.check(&path, &identity_key).await.is_err() {
        if let Some(counter) = RATE_LIMITED_TOTAL.get() {
            counter.with_label_values(&[&path]).inc();
        }
        tracing::warn!(target: "rate_limit", route=%path, identity=%identity_key, "rejected");
        let body = serde_json::json!({
            "code": "RATE_LIMITED",
            "message": "too many requests",
            "retry_after_ms": 1000u64,
        });
        let mut response = (StatusCode::TOO_MANY_REQUESTS, axum::Json(body)).into_response();
        response.headers_mut().insert(
            "Retry-After",
            HeaderValue::from_static("1"),
        );
        return response;
    }
    next.run(request).await
}

fn extract_identity_key(request: &Request<Body>) -> String {
    // 1. X-Real-IP header (set by nginx reverse proxy when present)
    if let Some(ip) = request
        .headers()
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
    {
        return format!("ip:{}", ip);
    }
    // 2. Peer socket address (ConnectInfo middleware — must be wired in main.rs
    //    via into_make_service_with_connect_info for this to be Some).
    if let Some(c) = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        return format!("ip:{}", c.0.ip());
    }
    // 3. Last-resort fallback: NEVER fall back to a shared literal ("ip:unknown")
    //    that would let every IP-less request share one bucket and starve
    //    legitimate traffic. Use a per-request token so anonymous requests
    //    do not collide. Operators should ensure ConnectInfo is wired up to
    //    avoid this fallback in production (see main.rs::serve).
    format!("anon:{}", uuid::Uuid::new_v4())
}
