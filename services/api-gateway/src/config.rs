use common::config::{RedisConfig, ServerConfig};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RouteConfig {
    pub upstream: String,
    pub downstream: String,
    pub service: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub ws: bool,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouteFile {
    pub route: Vec<RouteConfig>,
}

#[derive(Debug, Clone, Default)]
pub struct RouteTable {
    pub routes: Vec<RouteConfig>,
}

fn default_consul_addr() -> String {
    "http://127.0.0.1:8500".into()
}

fn default_jwks_url() -> String {
    "http://127.0.0.1:4000/.well-known/jwks.json".into()
}

fn default_issuer() -> String {
    "superpowers-token-server".into()
}

fn default_routes_dir() -> String {
    "/app/config/routes".into()
}

fn default_event_bus_url() -> String {
    "amqp://guest:guest@localhost:5672/%2f".into()
}

/// Default perm_epoch TTL in seconds (900 = 15min). MUST align with iam's
/// `access_token_ttl_minutes` (15) — the epoch only needs to outlive the
/// longest-lived access token it could invalidate.
fn default_perm_epoch_ttl_sec() -> u64 {
    900
}

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub redis: RedisConfig,

    #[serde(default = "default_consul_addr")]
    pub consul_addr: String,
    #[serde(default = "default_jwks_url")]
    pub jwks_url: String,
    #[serde(default = "default_issuer")]
    pub issuer: String,
    #[serde(default = "default_routes_dir")]
    pub routes_dir: String,

    #[serde(default = "default_event_bus_url")]
    pub event_bus_url: String,
    #[serde(default = "default_perm_epoch_ttl_sec")]
    pub perm_epoch_ttl_sec: u64,

    #[serde(default)]
    pub routes: Vec<RouteConfig>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".into(),
                port: 3000,
            },
            redis: RedisConfig::default(),
            consul_addr: "http://127.0.0.1:8500".into(),
            jwks_url: "http://127.0.0.1:4000/.well-known/jwks.json".into(),
            issuer: "superpowers-token-server".into(),
            routes_dir: default_routes_dir(),
            event_bus_url: default_event_bus_url(),
            perm_epoch_ttl_sec: default_perm_epoch_ttl_sec(),
            routes: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_config_parses_upstream_downstream() {
        let toml = r#"
            [[route]]
            upstream   = "/v1/users/{rest}"
            downstream = "/users/{rest}"
            service    = "user-service"
            public     = false
        "#;
        let cfg: RouteFile = toml::from_str(toml).unwrap();
        assert_eq!(cfg.route.len(), 1);
        assert_eq!(cfg.route[0].upstream, "/v1/users/{rest}");
        assert_eq!(cfg.route[0].downstream, "/users/{rest}");
        assert_eq!(cfg.route[0].service, "user-service");
        assert!(!cfg.route[0].public);
        assert!(!cfg.route[0].ws);
        assert!(cfg.route[0].headers.is_empty());
    }

    #[test]
    fn route_config_parses_ws_and_headers() {
        let toml = r#"
            [[route]]
            upstream   = "/v1/notification/ws"
            downstream = "/notification/ws"
            service    = "notification-service"
            ws         = true
            headers    = { "X-Source" = "gateway" }
        "#;
        let cfg: RouteFile = toml::from_str(toml).unwrap();
        let r = &cfg.route[0];
        assert!(r.ws);
        assert_eq!(r.headers.get("X-Source").map(String::as_str), Some("gateway"));
    }
}
