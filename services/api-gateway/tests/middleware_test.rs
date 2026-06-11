use api_gateway::middleware::is_public_route;
use api_gateway::RouteConfig;

fn route(match_pattern: &str, service: &str, public: bool) -> RouteConfig {
    RouteConfig {
        match_pattern: match_pattern.into(),
        service: service.into(),
        public,
        rewrite: vec![],
    }
}

#[test]
fn exact_match_public_is_allowed() {
    let routes = vec![route("/health", "gateway", true)];
    assert!(is_public_route(&routes, "/api/health"));
}

#[test]
fn exact_match_private_is_rejected() {
    let routes = vec![route("/users/me", "user-service", false)];
    assert!(!is_public_route(&routes, "/api/users/me"));
}

#[test]
fn wildcard_prefix_matches_subpaths() {
    let routes = vec![route("/auth/**", "auth-service", true)];
    assert!(is_public_route(&routes, "/api/auth/login"));
    assert!(is_public_route(&routes, "/api/auth/oauth/callback"));
    assert!(is_public_route(&routes, "/api/auth/"));
    assert!(is_public_route(&routes, "/api/auth"));
}

#[test]
fn wildcard_prefix_does_not_match_unrelated_path() {
    let routes = vec![route("/auth/**", "auth-service", true)];
    assert!(!is_public_route(&routes, "/api/users/me"));
    assert!(!is_public_route(&routes, "/api/author"));
}

#[test]
fn non_public_wildcard_blocks_subpaths() {
    let routes = vec![route("/api/**", "internal", false)];
    assert!(!is_public_route(&routes, "/api/api/users"));
    assert!(!is_public_route(&routes, "/api/api"));
}

#[test]
fn any_matching_public_route_grants_access() {
    let routes = vec![
        route("/users/**", "user-service", false),
        route("/auth/**", "auth-service", true),
    ];
    assert!(is_public_route(&routes, "/api/auth/login"));
    assert!(!is_public_route(&routes, "/api/users/me"));
}

#[test]
fn empty_route_table_denies_everything() {
    let routes: Vec<RouteConfig> = vec![];
    assert!(!is_public_route(&routes, "/api/health"));
    assert!(!is_public_route(&routes, "/"));
}

#[test]
fn path_outside_api_prefix_is_denied() {
    let routes = vec![route("/health", "gateway", true)];
    assert!(!is_public_route(&routes, "/health"));
    assert!(!is_public_route(&routes, "/api/other"));
}
