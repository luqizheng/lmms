//! token-service config loader.
//!
//! ```text
//! Priority: env > yaml file > Config::default()
//! ```
//!
//! Historical note (2026-07): earlier versions of this file used
//! `digua_common_config::Loader`, which layered `config::Environment::with_prefix
//! ("TOKEN").separator("_")` on top of the yaml source. That auto-binder
//! treats **every** `_` in an env var name as a nesting separator, so
//! `TOKEN_DATABASE_DB_TYPE` unbindingly resolved to path
//! `database.db.type` — never `database.db_type`. YAML with `db_type` +
//! env `TOKEN_DATABASE_DB_TYPE` would silently disagree, and serde would
//! then blame the yaml for a "missing field `db_type`".
//!
//! We now match the shape used by iam-service / notification-service:
//! parse yaml with serde, then explicitly overlay a hand-written list of
//! env vars. Failed env parses log + fall through instead of crashing.
//! The merged struct is then validated with a hard fail on missing
//! deployment-critical fields.
//!
//! The `apply_env_overrides` fn takes an env-source closure so it can be
//! unit-tested against a `HashMap` without touching the process-wide env
//! table. `main.rs` also uses this closure hook to layer optional
//! `config-service` fetched values *underneath* process env, so ops env
//! vars still win.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

pub const DEFAULT_CONFIG_PATH: &str = "config/config.yaml";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Mysql,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCfg {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseCfg {
    pub db_type: DbType,
    #[serde(default)]
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ConsulCfg {
    pub addr: String,
    pub service_name: String,
    pub service_id_file: String,
}

impl Default for ConsulCfg {
    fn default() -> Self {
        Self {
            addr: "http://localhost:8500".into(),
            service_name: "token-service".into(),
            service_id_file: "/tmp/token-service-id".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerCfg,
    pub database: DatabaseCfg,
    pub consul: ConsulCfg,
    pub jwt_private_key_path: String,
    pub jwt_public_key_path: String,
    pub issuer: String,
    pub default_access_ttl_sec: i64,
    pub default_refresh_ttl_sec: i64,
    pub default_resource_ttl_sec: i64,
    pub user_service_url: String,
}

impl Default for Config {
    fn default() -> Self {
        // Non-secret operational defaults only. Fields tied to a specific
        // deployment (database.url, user_service_url) or that could cause
        // silent misconfiguration if wrong (jwt key paths, issuer) start
        // empty and are enforced by `validate_all_fields_present` after
        // the env overlay has been applied.
        Self {
            server: ServerCfg {
                host: "0.0.0.0".into(),
                port: 4000,
            },
            database: DatabaseCfg {
                db_type: DbType::Mysql,
                url: String::new(),
                max_connections: 16,
            },
            consul: ConsulCfg::default(),
            jwt_private_key_path: String::new(),
            jwt_public_key_path: String::new(),
            issuer: String::new(),
            default_access_ttl_sec: 3600,
            default_refresh_ttl_sec: 2592000,
            default_resource_ttl_sec: 300,
            user_service_url: String::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Loader
// ─────────────────────────────────────────────────────────────────────────────

/// Pure yaml loader (no env). Kept for tests / tooling. Missing file =>
/// `Config::default()` (the default has enough non-secret knobs to boot a
/// dev instance if every deployment-critical field is provided via env).
pub fn load_config(path: &str) -> Result<Config> {
    match fs::read_to_string(path) {
        Ok(raw) => serde_yaml::from_str::<Config>(&raw)
            .with_context(|| format!("parse config {path}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                path = path,
                "config file not found; falling back to Config::default() (env is expected to fill required fields)"
            );
            Ok(Config::default())
        }
        Err(e) => Err(e).with_context(|| format!("read config {path}")),
    }
}

/// Load config from `path`, then apply env overrides via `env_lookup`.
///
/// The env-source closure lets callers layer additional sources under the
/// process env (e.g. main.rs merges a `config-service` fetched HashMap so
/// process env still wins the final say). The closure is checked for each
/// env-var name listed in `apply_env_overrides`.
pub fn load_config_with_env<F>(path: &str, env_lookup: F) -> Result<Config>
where
    F: Fn(&str) -> Option<String>,
{
    let mut cfg = load_config(path)?;
    apply_env_overrides(&mut cfg, &env_lookup);
    validate_all_fields_present(&cfg)?;
    Ok(cfg)
}

// ─────────────────────────────────────────────────────────────────────────────
// Env overrides
// ─────────────────────────────────────────────────────────────────────────────

/// Overwrites config fields from env. Malformed numeric values are logged
/// and the yaml value is kept, so a typo in one env var never turns into
/// a hard startup failure — `validate_all_fields_present` will catch true
/// absence later.
///
/// Env name convention: `TOKEN_<UPPER_YAML_PATH_WITH_SINGLE_UNDERSCORES>`,
/// treating `_` as *part of a field name*, not as a nesting separator.
/// This is the whole point of the hand-written overlay — see the file
/// header for the history.
pub fn apply_env_overrides<F>(cfg: &mut Config, env: &F)
where
    F: Fn(&str) -> Option<String>,
{
    // ── server ──
    if let Some(v) = env("TOKEN_SERVER_HOST") {
        cfg.server.host = v;
    }
    if let Some(v) = env("TOKEN_SERVER_PORT") {
        parse_or_warn("TOKEN_SERVER_PORT", &v, &mut cfg.server.port);
    }

    // ── database ──
    if let Some(v) = env("TOKEN_DATABASE_URL") {
        cfg.database.url = v;
    }
    if let Some(v) = env("TOKEN_DATABASE_MAX_CONNECTIONS") {
        parse_or_warn(
            "TOKEN_DATABASE_MAX_CONNECTIONS",
            &v,
            &mut cfg.database.max_connections,
        );
    }
    if let Some(v) = env("TOKEN_DATABASE_DB_TYPE") {
        match v.to_ascii_lowercase().as_str() {
            "mysql" => cfg.database.db_type = DbType::Mysql,
            other => tracing::warn!(
                key = "TOKEN_DATABASE_DB_TYPE",
                raw = other,
                "unknown db_type; keeping yaml value (only 'mysql' is supported)"
            ),
        }
    }

    // ── consul ──
    if let Some(v) = env("TOKEN_CONSUL_ADDR") {
        cfg.consul.addr = v;
    }
    if let Some(v) = env("TOKEN_CONSUL_SERVICE_NAME") {
        cfg.consul.service_name = v;
    }
    if let Some(v) = env("TOKEN_CONSUL_SERVICE_ID_FILE") {
        cfg.consul.service_id_file = v;
    }

    // ── jwt keys / issuer ──
    if let Some(v) = env("TOKEN_JWT_PRIVATE_KEY_PATH") {
        cfg.jwt_private_key_path = v;
    }
    if let Some(v) = env("TOKEN_JWT_PUBLIC_KEY_PATH") {
        cfg.jwt_public_key_path = v;
    }
    if let Some(v) = env("TOKEN_ISSUER") {
        cfg.issuer = v;
    }

    // ── ttl knobs ──
    if let Some(v) = env("TOKEN_DEFAULT_ACCESS_TTL_SEC") {
        parse_or_warn(
            "TOKEN_DEFAULT_ACCESS_TTL_SEC",
            &v,
            &mut cfg.default_access_ttl_sec,
        );
    }
    if let Some(v) = env("TOKEN_DEFAULT_REFRESH_TTL_SEC") {
        parse_or_warn(
            "TOKEN_DEFAULT_REFRESH_TTL_SEC",
            &v,
            &mut cfg.default_refresh_ttl_sec,
        );
    }
    if let Some(v) = env("TOKEN_DEFAULT_RESOURCE_TTL_SEC") {
        parse_or_warn(
            "TOKEN_DEFAULT_RESOURCE_TTL_SEC",
            &v,
            &mut cfg.default_resource_ttl_sec,
        );
    }

    // ── downstream service urls ──
    if let Some(v) = env("TOKEN_USER_SERVICE_URL") {
        cfg.user_service_url = v;
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
            "env override failed to parse; keeping yaml value"
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Merged-value validation
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies every deployment-required field has a non-empty value AFTER
/// env merge. Non-sensitive operational knobs (server.host/port, consul.*,
/// ttl_sec) still fall back to `Default`. See `services/token-service/
/// AGENTS.md` for the full env var table.
pub fn validate_all_fields_present(cfg: &Config) -> Result<()> {
    let mut missing: Vec<&'static str> = Vec::new();

    if cfg.database.url.trim().is_empty() {
        missing.push("database.url (TOKEN_DATABASE_URL or yaml)");
    }
    if cfg.jwt_private_key_path.trim().is_empty() {
        missing.push("jwt_private_key_path (TOKEN_JWT_PRIVATE_KEY_PATH or yaml)");
    }
    if cfg.jwt_public_key_path.trim().is_empty() {
        missing.push("jwt_public_key_path (TOKEN_JWT_PUBLIC_KEY_PATH or yaml)");
    }
    if cfg.issuer.trim().is_empty() {
        missing.push("issuer (TOKEN_ISSUER or yaml)");
    }
    if cfg.user_service_url.trim().is_empty() {
        missing.push("user_service_url (TOKEN_USER_SERVICE_URL or yaml)");
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "token-service config missing required fields after env merge: {}",
            missing.join(", ")
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

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
    fn parse_mysql_config() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 4000 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Mysql);
        assert_eq!(cfg.server.port, 4000);
        assert_eq!(cfg.database.max_connections, 8);
    }

    #[test]
    fn parse_invalid_db_type() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 4000 }
database:
  db_type: oracle
  url: "x"
  max_connections: 1
"#;
        assert!(serde_yaml::from_str::<Config>(yaml).is_err());
    }

    #[test]
    fn consul_default_applied() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 4000 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.consul.service_name, "token-service");
    }

    #[test]
    fn default_config_has_empty_sensitive_fields() {
        let cfg = Config::default();
        assert!(cfg.database.url.is_empty());
        assert!(cfg.jwt_private_key_path.is_empty());
        assert!(cfg.jwt_public_key_path.is_empty());
        assert!(cfg.issuer.is_empty());
        assert!(cfg.user_service_url.is_empty());
        assert_eq!(cfg.server.port, 4000);
        assert_eq!(cfg.default_access_ttl_sec, 3600);
    }

    #[test]
    fn validate_rejects_default_config() {
        let err = validate_all_fields_present(&Config::default()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("database.url"), "err: {msg}");
        assert!(msg.contains("jwt_private_key_path"), "err: {msg}");
        assert!(msg.contains("jwt_public_key_path"), "err: {msg}");
        assert!(msg.contains("issuer"), "err: {msg}");
        assert!(msg.contains("user_service_url"), "err: {msg}");
    }

    #[test]
    fn validate_passes_when_all_required_fields_set() {
        let cfg = Config {
            database: DatabaseCfg {
                db_type: DbType::Mysql,
                url: "mysql://x".into(),
                max_connections: 8,
            },
            jwt_private_key_path: "/keys/priv.pem".into(),
            jwt_public_key_path: "/keys/pub.pem".into(),
            issuer: "superpowers-token-service".into(),
            user_service_url: "http://iam:8081".into(),
            ..Config::default()
        };
        validate_all_fields_present(&cfg).expect("all fields present");
    }

    #[test]
    fn validate_rejects_whitespace_only_fields() {
        let cfg = Config {
            database: DatabaseCfg {
                db_type: DbType::Mysql,
                url: "   ".into(),
                max_connections: 8,
            },
            jwt_private_key_path: "\t".into(),
            jwt_public_key_path: "\n".into(),
            issuer: " ".into(),
            user_service_url: "".into(),
            ..Config::default()
        };
        let err = validate_all_fields_present(&cfg).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("database.url"), "err: {msg}");
        assert!(msg.contains("user_service_url"), "err: {msg}");
    }

    /// Regression: `db_type` used to disappear because
    /// `config::Environment::separator("_")` split `TOKEN_DATABASE_DB_TYPE`
    /// into `database.db.type`. Hand-written overlay must set the field
    /// verbatim.
    #[test]
    fn env_overlay_sets_db_type_verbatim() {
        let mut cfg = Config::default();
        cfg.database.url = "mysql://x".into(); // avoid the validator
        cfg.jwt_private_key_path = "p".into();
        cfg.jwt_public_key_path = "p".into();
        cfg.issuer = "i".into();
        cfg.user_service_url = "u".into();

        let env = env_from(&[("TOKEN_DATABASE_DB_TYPE", "mysql")]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.database.db_type, DbType::Mysql);
    }

    #[test]
    fn env_overlay_unknown_db_type_keeps_yaml_value() {
        let mut cfg = Config::default();
        cfg.database.db_type = DbType::Mysql;
        let env = env_from(&[("TOKEN_DATABASE_DB_TYPE", "postgres")]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(
            cfg.database.db_type,
            DbType::Mysql,
            "unknown value must be ignored"
        );
    }

    #[test]
    fn env_overlay_string_fields() {
        let mut cfg = Config::default();
        let env = env_from(&[
            ("TOKEN_SERVER_HOST", "127.0.0.1"),
            ("TOKEN_DATABASE_URL", "mysql://h/db"),
            ("TOKEN_JWT_PRIVATE_KEY_PATH", "/k/p.pem"),
            ("TOKEN_JWT_PUBLIC_KEY_PATH", "/k/pub.pem"),
            ("TOKEN_ISSUER", "acme"),
            ("TOKEN_USER_SERVICE_URL", "http://iam:8081"),
            ("TOKEN_CONSUL_ADDR", "http://consul:8500"),
            ("TOKEN_CONSUL_SERVICE_NAME", "token-x"),
        ]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.database.url, "mysql://h/db");
        assert_eq!(cfg.jwt_private_key_path, "/k/p.pem");
        assert_eq!(cfg.jwt_public_key_path, "/k/pub.pem");
        assert_eq!(cfg.issuer, "acme");
        assert_eq!(cfg.user_service_url, "http://iam:8081");
        assert_eq!(cfg.consul.addr, "http://consul:8500");
        assert_eq!(cfg.consul.service_name, "token-x");
    }

    #[test]
    fn env_overlay_numeric_fields() {
        let mut cfg = Config::default();
        let env = env_from(&[
            ("TOKEN_SERVER_PORT", "18082"),
            ("TOKEN_DATABASE_MAX_CONNECTIONS", "64"),
            ("TOKEN_DEFAULT_ACCESS_TTL_SEC", "1800"),
            ("TOKEN_DEFAULT_REFRESH_TTL_SEC", "600000"),
            ("TOKEN_DEFAULT_RESOURCE_TTL_SEC", "60"),
        ]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.server.port, 18082);
        assert_eq!(cfg.database.max_connections, 64);
        assert_eq!(cfg.default_access_ttl_sec, 1800);
        assert_eq!(cfg.default_refresh_ttl_sec, 600000);
        assert_eq!(cfg.default_resource_ttl_sec, 60);
    }

    #[test]
    fn malformed_numeric_env_keeps_yaml_value() {
        let mut cfg = Config::default();
        let yaml_port = cfg.server.port;
        let env = env_from(&[("TOKEN_SERVER_PORT", "not-a-number")]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.server.port, yaml_port);
    }

    #[test]
    fn empty_env_leaves_config_untouched() {
        let mut cfg = Config::default();
        let snapshot = format!("{:?}", cfg);
        apply_env_overrides(&mut cfg, &env_from(&[]));
        assert_eq!(format!("{:?}", cfg), snapshot);
    }

    #[test]
    fn load_config_with_env_end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(
            &path,
            r#"
server: { host: "0.0.0.0", port: 4000 }
database:
  db_type: mysql
  url: ""
  max_connections: 8
"#,
        )
        .unwrap();
        let env = env_from(&[
            ("TOKEN_DATABASE_URL", "mysql://u:p@h/db"),
            ("TOKEN_JWT_PRIVATE_KEY_PATH", "/k/priv.pem"),
            ("TOKEN_JWT_PUBLIC_KEY_PATH", "/k/pub.pem"),
            ("TOKEN_ISSUER", "acme"),
            ("TOKEN_USER_SERVICE_URL", "http://iam:8081"),
            ("TOKEN_SERVER_PORT", "18082"),
        ]);
        let cfg = load_config_with_env(path.to_str().unwrap(), &env).expect("load ok");
        assert_eq!(cfg.database.url, "mysql://u:p@h/db");
        assert_eq!(cfg.server.port, 18082);
        assert_eq!(cfg.database.db_type, DbType::Mysql);
    }

    #[test]
    fn load_config_with_env_fails_when_required_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(
            &path,
            r#"
server: { host: "0.0.0.0", port: 4000 }
database:
  db_type: mysql
  url: ""
  max_connections: 8
"#,
        )
        .unwrap();
        // No env at all → validation should fail with jwt / issuer / user_service_url absent.
        let err =
            load_config_with_env(path.to_str().unwrap(), &env_from(&[])).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("database.url"), "err: {msg}");
        assert!(msg.contains("issuer"), "err: {msg}");
    }

    #[test]
    fn load_config_missing_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.yaml");
        let cfg = load_config(path.to_str().unwrap()).expect("missing file → default");
        assert_eq!(cfg.server.port, 4000);
        assert!(cfg.database.url.is_empty());
    }
}
