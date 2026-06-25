//! Verifies notification-service's rate limit table is correctly wired.
//! Integration tests with real Redis live in scripts/dev/ when needed.

use notification_service::rest;

#[test]
fn default_limits_include_three_routes() {
    let limits = rest::default_limits();
    assert_eq!(limits.len(), 3);
}

#[test]
fn default_limits_have_anti_spam_on_create() {
    let limits = rest::default_limits();
    let create = limits
        .iter()
        .find(|l| l.route.contains("POST /notification/announcements"))
        .expect("create route limit must exist");
    // Anti-spam: 5 per minute (roadmap-mandated)
    assert_eq!(create.max, 5);
    assert_eq!(create.window_secs, 60);
}

#[test]
fn default_limits_have_publish_and_revoke() {
    let limits = rest::default_limits();
    let publish = limits
        .iter()
        .find(|l| l.route.contains("publish"))
        .expect("publish route limit must exist");
    let revoke = limits
        .iter()
        .find(|l| l.route.contains("revoke"))
        .expect("revoke route limit must exist");
    assert!(publish.max >= 10, "publish should allow at least 10/min");
    assert!(revoke.max >= 10, "revoke should allow at least 10/min");
}