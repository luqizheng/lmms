use digua_common::config::{RedisConfig, ServerConfig};
use digua_common_http::ResilienceConfig;
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
    /// 可选 per-route 单次发送超时（毫秒）。
    /// 当 `Some(ms)` 时，调到上游服务的 `ResilientClient.call_with_timeout` 用该值
    /// 覆盖单次发送与整体超时；缺省 `None` 时走全局 `ResilienceConfig.timeout_ms`。
    #[serde(default)]
    pub timeout_ms: Option<u64>,
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
    "superpowers-token-service".into()
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
    pub resilience: ResilienceConfig,

    #[serde(default)]
    pub routes: Vec<RouteConfig>,

    #[serde(default = "default_egress_map")]
    pub egress_map: HashMap<String, EgressTarget>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct EgressTarget {
    pub host: String,
    pub port: u16,
}

fn default_egress_map() -> HashMap<String, EgressTarget> {
    let mut m = HashMap::new();
    let svc = |host: &str| EgressTarget {
        host: host.into(),
        port: 8080,
    };
    m.insert("iam-service".into(), svc("iam-service"));
    m.insert("token-service".into(), svc("token-service"));
    m.insert("notification-service".into(), svc("notification-service"));
    m.insert("audit-service".into(), svc("audit-service"));
    m.insert("prue_backend".into(), svc("prue_backend"));
    // ai-pm 下游业务服务（2026-11-05）：pm 与 iam 一致挂网关对外，/v1/pm/* → pm-api:8090
    // （路由文件由 ai-pm infra 提供：config/api-gateway/routes/pm.router.toml）。
    m.insert(
        "pm-api".into(),
        EgressTarget {
            host: "pm-api".into(),
            port: 8090,
        },
    );
    m
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
            issuer: "superpowers-token-service".into(),
            routes_dir: default_routes_dir(),
            event_bus_url: default_event_bus_url(),
            perm_epoch_ttl_sec: default_perm_epoch_ttl_sec(),
            resilience: ResilienceConfig::default(),
            routes: Vec::new(),
            egress_map: default_egress_map(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Loader
// ─────────────────────────────────────────────────────────────────────────────
//
// Historical note (2026-07): earlier versions used the `config` crate's
// `Environment::with_prefix("GW").separator("__")` auto-binder to overlay
// env vars, forcing env-var names like `GW_SERVER__HOST` (with a double
// underscore separator so `redis_url`-shape fields survived splitting).
// That was one of two workarounds for the fact that automatic env→struct
// binders can't distinguish a `_` inside a field name from a `_` marking
// a nesting boundary. token-service used the other (single-underscore)
// half of that dilemma and silently lost `db_type` to it. We unified
// both services on the same hand-written-overlay shape now used by
// iam-service / notification-service. Env var names are ordinary single
// underscore (`GW_SERVER_HOST`, `GW_REDIS_URL`, `GW_REDIS_PREFIX`).

/// Load `path` (any format supported by the `config` crate: yaml, toml,
/// json). Missing file falls back to `GatewayConfig::default()` — same
/// behaviour as before, keeping dev / single-file deploys non-fatal.
///
/// Accepts both relative paths (e.g. `"config/api-gateway"`) and absolute
/// paths (e.g. `/etc/gateway/api-gateway.toml`). Uses `File::from` so the
/// format is auto-detected from the file extension.
pub fn load_config(path: impl AsRef<std::path::Path>) -> Result<GatewayConfig, String> {
    let builder =
        ::config::Config::builder().add_source(::config::File::from(path.as_ref()).required(false));
    let cfg = builder
        .build()
        .map_err(|e| format!("api-gateway config build failed: {e}"))?
        .try_deserialize::<GatewayConfig>()
        .map_err(|e| format!("api-gateway config parse failed: {e}"))?;
    Ok(cfg)
}

/// Load config from `path`, then overlay env overrides via `env_lookup`.
/// The env-source closure lets tests exercise the overlay against a
/// `HashMap` without touching the process-wide env table.
pub fn load_config_with_env<F>(
    path: impl AsRef<std::path::Path>,
    env_lookup: F,
) -> Result<GatewayConfig, String>
where
    F: Fn(&str) -> Option<String>,
{
    let mut cfg = load_config(path)?;
    apply_env_overrides(&mut cfg, &env_lookup);
    Ok(cfg)
}

/// Overwrites config fields from env. Malformed numeric values are logged
/// and the file value is kept. Env name convention: `GW_<UPPER_YAML_PATH
/// _WITH_SINGLE_UNDERSCORES>`, treating `_` as *part of a field name*,
/// never as a nesting separator.
pub fn apply_env_overrides<F>(cfg: &mut GatewayConfig, env: &F)
where
    F: Fn(&str) -> Option<String>,
{
    // ── server ──
    if let Some(v) = env("GW_SERVER_HOST") {
        cfg.server.host = v;
    }
    if let Some(v) = env("GW_SERVER_PORT") {
        parse_or_warn("GW_SERVER_PORT", &v, &mut cfg.server.port);
    }

    // ── redis ──
    if let Some(v) = env("GW_REDIS_URL") {
        cfg.redis.url = v;
    }
    if let Some(v) = env("GW_REDIS_PREFIX") {
        cfg.redis.prefix = v;
    }

    // ── discovery / jwks / issuer / routes-dir ──
    if let Some(v) = env("GW_CONSUL_ADDR") {
        cfg.consul_addr = v;
    }
    if let Some(v) = env("GW_JWKS_URL") {
        cfg.jwks_url = v;
    }
    if let Some(v) = env("GW_ISSUER") {
        cfg.issuer = v;
    }
    if let Some(v) = env("GW_ROUTES_DIR") {
        cfg.routes_dir = v;
    }

    // ── event-bus / perm-epoch ──
    if let Some(v) = env("GW_EVENT_BUS_URL") {
        cfg.event_bus_url = v;
    }
    if let Some(v) = env("GW_PERM_EPOCH_TTL_SEC") {
        parse_or_warn("GW_PERM_EPOCH_TTL_SEC", &v, &mut cfg.perm_epoch_ttl_sec);
    }
}

fn parse_or_warn<T: std::str::FromStr>(key: &str, raw: &str, slot: &mut T)
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match raw.parse::<T>() {
        Ok(v) => *slot = v,
        Err(e) => tracing::warn!(
            key = key,
            raw = raw,
            error = %e,
            "env override failed to parse; keeping file value"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

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
        assert_eq!(
            r.headers.get("X-Source").map(String::as_str),
            Some("gateway")
        );
    }

    #[test]
    fn route_config_parses_timeout_ms_some() {
        let toml = r#"
            [[route]]
            upstream    = "/v1/ai/parse-requirement"
            downstream  = "/ai/parse-requirement"
            service     = "pm-platform"
            timeout_ms  = 90000
        "#;
        let cfg: RouteFile = toml::from_str(toml).unwrap();
        assert_eq!(cfg.route[0].timeout_ms, Some(90000));
    }

    #[test]
    fn route_config_parses_timeout_ms_absent_defaults_to_none() {
        let toml = r#"
            [[route]]
            upstream   = "/v1/iam/users/{rest}"
            downstream = "/iam/users/{rest}"
            service    = "iam-service"
        "#;
        let cfg: RouteFile = toml::from_str(toml).unwrap();
        assert_eq!(cfg.route[0].timeout_ms, None);
    }

    #[test]
    fn env_overlay_string_and_numeric_fields() {
        let mut cfg = GatewayConfig::default();
        let env = env_from(&[
            ("GW_SERVER_HOST", "127.0.0.1"),
            ("GW_SERVER_PORT", "13000"),
            ("GW_REDIS_URL", "redis://r/2"),
            ("GW_REDIS_PREFIX", "gw:"),
            ("GW_CONSUL_ADDR", "http://consul:8500"),
            (
                "GW_JWKS_URL",
                "http://token-service:8082/.well-known/jwks.json",
            ),
            ("GW_ISSUER", "acme"),
            ("GW_ROUTES_DIR", "/etc/gw/routes"),
            ("GW_EVENT_BUS_URL", "amqp://user:pw@rmq:5672/%2f"),
            ("GW_PERM_EPOCH_TTL_SEC", "1200"),
        ]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.server.port, 13000);
        assert_eq!(cfg.redis.url, "redis://r/2");
        assert_eq!(cfg.redis.prefix, "gw:");
        assert_eq!(cfg.consul_addr, "http://consul:8500");
        assert_eq!(
            cfg.jwks_url,
            "http://token-service:8082/.well-known/jwks.json"
        );
        assert_eq!(cfg.issuer, "acme");
        assert_eq!(cfg.routes_dir, "/etc/gw/routes");
        assert_eq!(cfg.event_bus_url, "amqp://user:pw@rmq:5672/%2f");
        assert_eq!(cfg.perm_epoch_ttl_sec, 1200);
    }

    /// Regression: `redis.prefix` used to require `GW_REDIS__PREFIX`
    /// (double underscore) because the `config` crate's env auto-binder
    /// treated a single `_` as a nesting boundary and would have split
    /// `GW_REDIS_URL` into `redis.url`… but also `GW_REDIS_PREFIX` into
    /// `redis.prefix`. The hand-written overlay accepts the single-`_`
    /// form directly, matching the shape used by every other service.
    #[test]
    fn env_overlay_single_underscore_prefix_works() {
        let mut cfg = GatewayConfig::default();
        let env = env_from(&[("GW_REDIS_PREFIX", "superpowers:")]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.redis.prefix, "superpowers:");
    }

    #[test]
    fn malformed_numeric_env_keeps_file_value() {
        let mut cfg = GatewayConfig::default();
        let file_port = cfg.server.port;
        let env = env_from(&[("GW_SERVER_PORT", "abc")]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.server.port, file_port);
    }

    #[test]
    fn empty_env_leaves_config_untouched() {
        let mut cfg = GatewayConfig::default();
        let snapshot = format!("{:?}", cfg);
        apply_env_overrides(&mut cfg, &env_from(&[]));
        assert_eq!(format!("{:?}", cfg), snapshot);
    }

    #[test]
    fn load_config_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist");
        let cfg = load_config(path.to_str().unwrap()).expect("missing file → default");
        assert_eq!(cfg.server.port, 3000);
        assert_eq!(cfg.redis.prefix, "superpowers:");
    }

    #[test]
    fn load_config_with_env_end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api-gateway.toml");
        // Root-level keys must precede any [table]; toml groups everything
        // after `[server]` under the server table.
        std::fs::write(
            &path,
            r#"
jwks_url = "http://from-file/jwks"

[server]
host = "0.0.0.0"
port = 3000

[redis]
url = "redis://from-file:6379"
prefix = "file:"
"#,
        )
        .unwrap();
        let env = env_from(&[
            ("GW_REDIS_URL", "redis://from-env:6379"),
            ("GW_ISSUER", "env-issuer"),
        ]);
        let cfg = load_config_with_env(path.to_str().unwrap(), &env).expect("load ok");
        // env wins over file, file wins over hardcoded default.
        assert_eq!(cfg.redis.url, "redis://from-env:6379");
        assert_eq!(cfg.redis.prefix, "file:");
        assert_eq!(cfg.jwks_url, "http://from-file/jwks");
        assert_eq!(cfg.issuer, "env-issuer");
    }
}
