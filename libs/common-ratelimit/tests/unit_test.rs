//! Unit tests for digua_common-ratelimit that don't need Redis.

use digua_common_ratelimit::{Limit, RateLimiter, DEFAULT_LIMIT};

// We can't actually construct a RateLimiter in unit tests (needs Redis),
// but we can test the limit_for logic with a mock if we structure it right.
// For now we test the Limit type and constants.

#[test]
fn limit_new_sets_fields() {
    let l = Limit::new("GET /foo", 100, 60);
    assert_eq!(l.route, "GET /foo");
    assert_eq!(l.max, 100);
    assert_eq!(l.window_secs, 60);
}

#[test]
fn default_limit_is_60_per_minute() {
    assert_eq!(DEFAULT_LIMIT.max, 60);
    assert_eq!(DEFAULT_LIMIT.window_secs, 60);
}

#[test]
fn limit_is_const_constructible() {
    // Verify that const fn works at compile time
    const L: Limit = Limit::new("X", 5, 60);
    assert_eq!(L.max, 5);
}

// limit_for tests are on the RateLimiter struct which requires Redis,
// so they're covered by integration tests in
// services/notification-service/tests/rate_limit_integration_test.rs
// (will be added if/when we want docker-based test runner for that case)

#[test]
fn limit_route_string_convention() {
    // Verify route key format: METHOD path
    // This is the convention used in notification-service default_limits()
    let l = Limit::new("POST /notification/announcements", 5, 60);
    assert!(l.route.starts_with("POST "));
    assert!(l.route.contains("/notification/"));
}
