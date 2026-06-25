use crate::AppState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use prometheus::IntCounterVec;
use serde::Deserialize;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};

/// Prometheus counter for rate-limited requests.
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
    // Pattern can be a literal path or a prefix ending with '*'
    if let Some(prefix) = pattern.strip_suffix('*') {
        path.starts_with(prefix)
    } else {
        pattern == path
    }
}

/// Per-route-token-bucket limiter keyed by (route, identity).
type Limiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

#[derive(Clone)]
pub struct RouteLimiterSet {
    limiters: Arc<HashMap<String, Arc<Limiter>>>,
    default: Arc<Limiter>,
}

impl RouteLimiterSet {
    pub fn from_config(cfg: &RateLimitConfig) -> Self {
        let mut limiters = HashMap::new();
        for (pattern, rule) in &cfg.routes {
            let quota = Quota::per_second(NonZeroU32::new(rule.requests_per_second).unwrap())
                .allow_burst(NonZeroU32::new(rule.burst).unwrap());
            limiters.insert(pattern.clone(), Arc::new(RateLimiter::direct(quota)));
        }
        let default_q = Quota::per_second(
            NonZeroU32::new(cfg.defaults.requests_per_second).unwrap(),
        )
        .allow_burst(NonZeroU32::new(cfg.defaults.burst).unwrap());
        Self {
            limiters: Arc::new(limiters),
            default: Arc::new(RateLimiter::direct(default_q)),
        }
    }

    pub fn check(&self, route_pattern: &str, _key: &str) -> Result<(), ()> {
        let limiter = self
            .limiters
            .iter()
            .find(|(p, _)| path_matches(p, route_pattern))
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| self.default.clone());
        limiter.check().map_err(|_| ())
    }
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let identity_key = extract_identity_key(&request);

    if state.route_limiters.check(&path, &identity_key).is_err() {
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
            axum::http::HeaderValue::from_static("1"),
        );
        return response;
    }
    next.run(request).await
}

fn extract_identity_key(request: &Request<Body>) -> String {
    // Prefer the user_id set by auth_middleware in extensions.
    if let Some(claims) = request.extensions().get::<crate::JwtClaims>() {
        return format!("user:{}", claims.sub);
    }
    // Fall back to X-Real-IP set by nginx.
    if let Some(ip) = request
        .headers()
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
    {
        return format!("ip:{}", ip);
    }
    // Last resort: peer address.
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| format!("ip:{}", c.0.ip()))
        .unwrap_or_else(|| "ip:unknown".to_string())
}