//! Unit tests for the rate-limit config + limiter math.
//! Integration tests (against running gateway) live in `tests/rate_limit_test.rs`.

use api_gateway::middleware::rate_limit::{RateLimitConfig, RateLimitRule, RouteLimiterSet};
use std::collections::HashMap;

#[test]
fn rule_for_returns_route_specific_match() {
    let mut routes = HashMap::new();
    routes.insert(
        "/v1/iam/auth/login".to_string(),
        RateLimitRule {
            requests_per_second: 5,
            burst: 10,
        },
    );
    let cfg = RateLimitConfig {
        defaults: RateLimitRule {
            requests_per_second: 100,
            burst: 200,
        },
        routes,
    };
    let rule = cfg.rule_for("/v1/iam/auth/login");
    assert_eq!(rule.requests_per_second, 5);
}

#[test]
fn rule_for_falls_back_to_defaults() {
    let cfg = RateLimitConfig::default();
    let rule = cfg.rule_for("/v1/anything");
    assert_eq!(rule.requests_per_second, 100);
}

#[test]
fn rule_for_supports_wildcard_suffix() {
    let mut routes = HashMap::new();
    routes.insert(
        "/v1/pure/*".to_string(),
        RateLimitRule {
            requests_per_second: 20,
            burst: 40,
        },
    );
    let cfg = RateLimitConfig {
        defaults: RateLimitRule {
            requests_per_second: 100,
            burst: 200,
        },
        routes,
    };
    assert_eq!(cfg.rule_for("/v1/pure/menu-configs").requests_per_second, 20);
    assert_eq!(cfg.rule_for("/v1/pure/anything/deep").requests_per_second, 20);
    // Non-matching path falls back
    assert_eq!(cfg.rule_for("/v1/iam/users").requests_per_second, 100);
}

#[test]
fn limiter_allows_within_burst_then_throttles() {
    let cfg = RateLimitConfig {
        defaults: RateLimitRule {
            requests_per_second: 1,
            burst: 2,
        },
        routes: HashMap::new(),
    };
    let set = RouteLimiterSet::from_config(&cfg);

    // Burst of 2 should succeed
    assert!(set.check("/v1/foo", "user:1").is_ok());
    assert!(set.check("/v1/foo", "user:1").is_ok());
    // Third immediate request should fail (bucket exhausted)
    assert!(set.check("/v1/foo", "user:1").is_err());
}

#[test]
fn limiter_is_per_route_independent() {
    let mut routes = HashMap::new();
    routes.insert(
        "/v1/a".to_string(),
        RateLimitRule {
            requests_per_second: 1,
            burst: 1,
        },
    );
    routes.insert(
        "/v1/b".to_string(),
        RateLimitRule {
            requests_per_second: 100,
            burst: 200,
        },
    );
    let cfg = RateLimitConfig {
        defaults: RateLimitRule {
            requests_per_second: 100,
            burst: 200,
        },
        routes,
    };
    let set = RouteLimiterSet::from_config(&cfg);

    // /v1/a is exhausted
    assert!(set.check("/v1/a", "u").is_ok());
    assert!(set.check("/v1/a", "u").is_err());
    // /v1/b unaffected
    for _ in 0..10 {
        assert!(set.check("/v1/b", "u").is_ok());
    }
}