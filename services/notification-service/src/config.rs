//! Config loader with three-tier semantics (mirrors iam-service):
//!
//! ```text
//! Priority: env > yaml file > (fail — no hardcoded fallback for URLs)
//! ```
//!
//! On first boot, when `CONFIG_PATH` does not exist, `DEFAULT_YAML` (embedded
//! via `include_str!`) is atomically written to that path. `apply_env_overrides`
//! then overwrites fields from a well-known set of env vars, and
//! `validate_all_fields_present` fails startup if any required field is blank
//! after the merge. `apply_env_overrides` takes an env-source closure so it can
//! be unit tested with a `HashMap` without touching the process env table.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

pub const DEFAULT_YAML: &str = include_str!("../config/config.default.yaml");

pub const DEFAULT_CONFIG_PATH: &str = "/etc/notification-service/config.yaml";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerCfg {
    pub http_addr: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MysqlCfg {
    #[serde(default)]
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RedisCfg {
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchedulerCfg {
    pub tick_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogCfg {
    pub level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
            service_name: "notification-service".into(),
            service_id_file: "/tmp/notification-service-id".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct EventBusCfg {
    pub url: String,
}

impl Default for EventBusCfg {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@localhost:5672/%2f".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerCfg,
    pub mysql: MysqlCfg,
    pub redis: RedisCfg,
    pub scheduler: SchedulerCfg,
    pub log: LogCfg,
    pub consul: ConsulCfg,
    #[serde(default)]
    pub event_bus: EventBusCfg,
    #[serde(default = "default_iam_base_url")]
    pub iam_base_url: String,
}

fn default_iam_base_url() -> String {
    "http://127.0.0.1:9081".to_string()
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
        Self::load_with_env(&path, |k| std::env::var(k).ok())
    }

    pub fn load_with_env<F>(config_path: &str, env_lookup: F) -> anyhow::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let path = Path::new(config_path);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match seed_if_absent(path) {
                Ok(true) => tracing::info!(
                    path = %config_path,
                    "seeded config.yaml from embedded default"
                ),
                Ok(false) => {}
                Err(e) => tracing::warn!(
                    error = %e,
                    path = %config_path,
                    "seed failed; falling back to embedded default as yaml input"
                ),
            }
        }

        let yaml_content = fs::read_to_string(path).unwrap_or_else(|e| {
            tracing::warn!(
                error = %e,
                path = %config_path,
                "config file unreadable; using embedded default as yaml input"
            );
            DEFAULT_YAML.to_string()
        });
        let mut cfg: AppConfig = serde_yaml::from_str(&yaml_content)?;

        apply_env_overrides(&mut cfg, &env_lookup);
        validate_all_fields_present(&cfg)?;
        Ok(cfg)
    }
}

fn seed_if_absent(path: &Path) -> std::io::Result<bool> {
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => {
            f.write_all(DEFAULT_YAML.as_bytes())?;
            Ok(true)
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn apply_env_overrides<F>(cfg: &mut AppConfig, env: &F)
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(v) = env("SERVER_HTTP_ADDR") {
        cfg.server.http_addr = v;
    }
    if let Some(v) = env("SERVER_PORT") {
        rebind_port(&mut cfg.server.http_addr, &v);
    }

    if let Some(v) = env("DATABASE_URL") {
        cfg.mysql.url = v;
    }
    if let Some(v) = env("DATABASE_MAX_CONNECTIONS") {
        parse_or_warn(
            "DATABASE_MAX_CONNECTIONS",
            &v,
            &mut cfg.mysql.max_connections,
        );
    }

    if let Some(v) = env("REDIS_URL") {
        cfg.redis.url = v;
    }

    if let Some(v) = env("SCHEDULER_TICK_SECS") {
        parse_or_warn("SCHEDULER_TICK_SECS", &v, &mut cfg.scheduler.tick_secs);
    }

    if let Some(v) = env("RUST_LOG") {
        cfg.log.level = v;
    }

    if let Some(v) = env("CONSUL_ADDR") {
        cfg.consul.addr = v;
    }
    if let Some(v) = env("CONSUL_SERVICE_NAME") {
        cfg.consul.service_name = v;
    }
    if let Some(v) = env("CONSUL_SERVICE_ID_FILE") {
        cfg.consul.service_id_file = v;
    }

    if let Some(v) = env("EVENT_BUS_URL") {
        cfg.event_bus.url = v;
    }

    if let Some(v) = env("IAM_BASE_URL") {
        cfg.iam_base_url = v;
    }
}

/// Rewrites the port half of a `host:port` addr. Keeps the host part intact so
/// SERVER_PORT alone can be flipped without also setting SERVER_HTTP_ADDR.
/// Malformed input (no `:`, non-numeric port) is logged and dropped.
fn rebind_port(http_addr: &mut String, new_port: &str) {
    if new_port.parse::<u16>().is_err() {
        tracing::warn!(
            key = "SERVER_PORT",
            raw = new_port,
            "env override not a valid u16; keeping yaml value"
        );
        return;
    }
    match http_addr.rsplit_once(':') {
        Some((host, _)) => *http_addr = format!("{host}:{new_port}"),
        None => tracing::warn!(
            addr = %http_addr,
            "server.http_addr has no ':'; cannot apply SERVER_PORT override"
        ),
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

pub fn validate_all_fields_present(cfg: &AppConfig) -> anyhow::Result<()> {
    let mut missing: Vec<&'static str> = Vec::new();

    if cfg.server.http_addr.trim().is_empty() {
        missing.push("server.http_addr (SERVER_HTTP_ADDR/SERVER_PORT or yaml)");
    }
    if cfg.mysql.url.trim().is_empty() {
        missing.push("mysql.url (DATABASE_URL or yaml)");
    }
    if cfg.redis.url.trim().is_empty() {
        missing.push("redis.url (REDIS_URL or yaml)");
    }
    if cfg.log.level.trim().is_empty() {
        missing.push("log.level (RUST_LOG or yaml)");
    }
    if cfg.consul.addr.trim().is_empty() {
        missing.push("consul.addr (CONSUL_ADDR or yaml)");
    }
    if cfg.consul.service_name.trim().is_empty() {
        missing.push("consul.service_name (CONSUL_SERVICE_NAME or yaml)");
    }
    if cfg.event_bus.url.trim().is_empty() {
        missing.push("event_bus.url (EVENT_BUS_URL or yaml)");
    }
    if cfg.iam_base_url.trim().is_empty() {
        missing.push("iam_base_url (IAM_BASE_URL or yaml)");
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "notification-service config missing required fields after env merge: {}",
            missing.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    #[test]
    fn embedded_default_yaml_deserializes() {
        let cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).expect("DEFAULT_YAML parses");
        assert_eq!(cfg.server.http_addr, "0.0.0.0:8083");
        assert!(cfg.mysql.url.is_empty());
        assert!(cfg.redis.url.is_empty());
        assert_eq!(cfg.scheduler.tick_secs, 60);
        assert_eq!(cfg.consul.service_name, "notification-service");
    }

    #[test]
    fn apply_env_overrides_string_and_url_fields() {
        let mut cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let env = env_from(&[
            ("DATABASE_URL", "mysql://u:p@h/db"),
            ("REDIS_URL", "redis://h:6379"),
            ("EVENT_BUS_URL", "amqp://x/y"),
            ("IAM_BASE_URL", "http://iam:9081"),
            ("CONSUL_ADDR", "http://consul:9500"),
            ("CONSUL_SERVICE_NAME", "ntf-x"),
            ("SERVER_HTTP_ADDR", "0.0.0.0:9999"),
        ]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.mysql.url, "mysql://u:p@h/db");
        assert_eq!(cfg.redis.url, "redis://h:6379");
        assert_eq!(cfg.event_bus.url, "amqp://x/y");
        assert_eq!(cfg.iam_base_url, "http://iam:9081");
        assert_eq!(cfg.consul.addr, "http://consul:9500");
        assert_eq!(cfg.consul.service_name, "ntf-x");
        assert_eq!(cfg.server.http_addr, "0.0.0.0:9999");
    }

    #[test]
    fn server_port_env_rebinds_port_only() {
        let mut cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let env = env_from(&[("SERVER_PORT", "12345")]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.server.http_addr, "0.0.0.0:12345");
    }

    #[test]
    fn server_port_env_wins_over_http_addr_only_when_applied_later() {
        let mut cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let env = env_from(&[
            ("SERVER_HTTP_ADDR", "127.0.0.1:8888"),
            ("SERVER_PORT", "9999"),
        ]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.server.http_addr, "127.0.0.1:9999");
    }

    #[test]
    fn malformed_numeric_env_keeps_yaml_value() {
        let mut cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let yaml_ticks = cfg.scheduler.tick_secs;
        let env = env_from(&[
            ("SERVER_PORT", "not-a-number"),
            ("SCHEDULER_TICK_SECS", "boom"),
        ]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.server.http_addr, "0.0.0.0:8083");
        assert_eq!(cfg.scheduler.tick_secs, yaml_ticks);
    }

    #[test]
    fn empty_env_leaves_config_untouched() {
        let mut cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let snapshot = format!("{:?}", cfg);
        apply_env_overrides(&mut cfg, &env_from(&[]));
        assert_eq!(format!("{:?}", cfg), snapshot);
    }

    #[test]
    fn validate_rejects_missing_mysql_and_redis_url() {
        let cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let err = validate_all_fields_present(&cfg).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("mysql.url"), "err: {msg}");
        assert!(msg.contains("redis.url"), "err: {msg}");
    }

    #[test]
    fn validate_passes_when_urls_provided_via_env() {
        let mut cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let env = env_from(&[("DATABASE_URL", "mysql://x"), ("REDIS_URL", "redis://x")]);
        apply_env_overrides(&mut cfg, &env);
        validate_all_fields_present(&cfg).expect("all fields present after env merge");
    }

    #[test]
    fn validate_rejects_whitespace_only_url() {
        let mut cfg: AppConfig = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        cfg.mysql.url = "   ".into();
        cfg.redis.url = "\n".into();
        let err = validate_all_fields_present(&cfg).unwrap_err();
        assert!(err.to_string().contains("mysql.url"));
    }

    #[test]
    fn seed_writes_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        assert!(seed_if_absent(&path).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), DEFAULT_YAML);
    }

    #[test]
    fn seed_returns_false_on_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "server:\n  http_addr: preset\n").unwrap();
        assert!(!seed_if_absent(&path).unwrap());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "server:\n  http_addr: preset\n"
        );
    }

    #[test]
    fn seed_race_only_one_writer_wins() {
        let dir = tempfile::tempdir().unwrap();
        let path = Arc::new(dir.path().join("config.yaml"));
        let n = 8;
        let barrier = Arc::new(Barrier::new(n));
        let handles: Vec<_> = (0..n)
            .map(|_| {
                let path = Arc::clone(&path);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    seed_if_absent(&path).unwrap()
                })
            })
            .collect();
        let wins: usize = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|w| *w)
            .count();
        assert_eq!(wins, 1);
        assert_eq!(fs::read_to_string(path.as_path()).unwrap(), DEFAULT_YAML);
    }

    #[test]
    fn load_with_env_end_to_end_seed_then_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/dir/config.yaml");
        let env = env_from(&[
            ("DATABASE_URL", "mysql://u:p@h/db"),
            ("REDIS_URL", "redis://r"),
            ("SERVER_PORT", "12345"),
        ]);
        let cfg =
            AppConfig::load_with_env(path.to_str().unwrap(), env).expect("load should succeed");
        assert_eq!(cfg.mysql.url, "mysql://u:p@h/db");
        assert_eq!(cfg.redis.url, "redis://r");
        assert_eq!(cfg.server.http_addr, "0.0.0.0:12345");
        assert!(path.exists());
    }

    #[test]
    fn load_with_env_fails_when_urls_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let err = AppConfig::load_with_env(path.to_str().unwrap(), env_from(&[])).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("mysql.url"), "err: {msg}");
        assert!(msg.contains("redis.url"), "err: {msg}");
    }
}
