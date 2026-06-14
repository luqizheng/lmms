use common::config::{RedisConfig, ServerConfig};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RouteConfig {
    #[serde(rename = "match")]
    pub match_pattern: String,
    pub service: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub rewrite: Vec<RewriteSpec>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum RewriteSpec {
    #[serde(rename = "strip_prefix")]
    StripPrefix { prefix: String },
    #[serde(rename = "redirect")]
    Redirect { status: u16, to: String },
    #[serde(rename = "header_set")]
    HeaderSet { name: String, value: String },
    #[serde(rename = "header_add")]
    HeaderAdd { name: String, value: String },
    #[serde(rename = "path_param")]
    PathParam { from: String, to: String },
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
            routes: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_config_parses_match_and_service() {
        let toml = r#"
            [[route]]
            match = "/users/**"
            service = "user-service"
            public = false
        "#;
        let cfg: RouteFile = toml::from_str(toml).unwrap();
        assert_eq!(cfg.route.len(), 1);
        assert_eq!(cfg.route[0].match_pattern, "/users/**");
        assert_eq!(cfg.route[0].service, "user-service");
        assert!(!cfg.route[0].public);
        assert!(cfg.route[0].rewrite.is_empty());
    }

    #[test]
    fn route_config_parses_rewrite_chain() {
        let toml = r#"
            [[route]]
            match = "/auth/logout"
            service = "auth-service"
            public = true
            rewrite = [
              { type = "header_set", name = "X-Auth-Audit", value = "logout" },
              { type = "strip_prefix", prefix = "/api" }
            ]
        "#;
        let cfg: RouteFile = toml::from_str(toml).unwrap();
        let r = &cfg.route[0];
        assert_eq!(r.rewrite.len(), 2);
        assert!(matches!(r.rewrite[0], RewriteSpec::HeaderSet { .. }));
        assert!(matches!(r.rewrite[1], RewriteSpec::StripPrefix { .. }));
    }

    #[test]
    fn rewrite_redirect_has_status_and_to() {
        let spec = RewriteSpec::Redirect {
            status: 301,
            to: "/new".into(),
        };
        match spec {
            RewriteSpec::Redirect { status, to } => {
                assert_eq!(status, 301);
                assert_eq!(to, "/new");
            }
            _ => panic!("expected redirect"),
        }
    }
}
