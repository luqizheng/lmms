use api_gateway::middleware::is_public_route;
use api_gateway::RouteConfig;

fn route(upstream: &str, downstream: &str, service: &str, public: bool) -> RouteConfig {
    RouteConfig {
        upstream: upstream.into(),
        downstream: downstream.into(),
        service: service.into(),
        public,
        ws: false,
        headers: Default::default(),
        timeout_ms: None,
    }
}

#[test]
fn exact_match_public_is_allowed() {
    let routes = vec![route("/health", "/health", "gateway", true)];
    assert!(is_public_route(&routes, "/health"));
}

#[test]
fn exact_match_private_is_rejected() {
    let routes = vec![route("/users/me", "/users/me", "user-service", false)];
    assert!(!is_public_route(&routes, "/users/me"));
}

#[test]
fn wildcard_prefix_matches_subpaths() {
    let routes = vec![route("/auth/{rest}", "/auth/{rest}", "auth-service", true)];
    assert!(is_public_route(&routes, "/auth/login"));
    assert!(is_public_route(&routes, "/auth/oauth/callback"));
}

#[test]
fn wildcard_prefix_does_not_match_unrelated_path() {
    let routes = vec![route("/auth/{rest}", "/auth/{rest}", "auth-service", true)];
    assert!(!is_public_route(&routes, "/users/me"));
    assert!(!is_public_route(&routes, "/author"));
}

#[test]
fn non_public_wildcard_blocks_subpaths() {
    let routes = vec![route("/api/{rest}", "/api/{rest}", "internal", false)];
    assert!(!is_public_route(&routes, "/api/users"));
}

#[test]
fn any_matching_public_route_grants_access() {
    let routes = vec![
        route("/users/{rest}", "/users/{rest}", "user-service", false),
        route("/auth/{rest}", "/auth/{rest}", "auth-service", true),
    ];
    assert!(is_public_route(&routes, "/auth/login"));
    assert!(!is_public_route(&routes, "/users/me"));
}

#[test]
fn empty_route_table_denies_everything() {
    let routes: Vec<RouteConfig> = vec![];
    assert!(!is_public_route(&routes, "/health"));
    assert!(!is_public_route(&routes, "/"));
}

#[test]
fn path_outside_pattern_is_denied() {
    let routes = vec![route("/health", "/health", "gateway", true)];
    assert!(!is_public_route(&routes, "/other"));
}
