//! Config loader with three-tier semantics:
//!
//! ```text
//! Priority: env > yaml file > (fail — no hardcoded fallback)
//! ```
//!
//! - **Seed on first boot**: If `CONFIG_PATH` does not exist, the embedded
//!   `config.default.yaml` (compile-time `include_str!`) is atomically written
//!   to that path. Subsequent boots read the seeded (possibly ops-edited) file.
//! - **Env overrides**: After the yaml is parsed, `apply_env_overrides` walks
//!   a well-known set of env vars and overwrites the corresponding fields.
//! - **Merge-time validation**: `validate_all_fields_present` fails startup if
//!   any required field is missing or empty after the merge. There is **no**
//!   hardcoded fallback for `database.url` / `redis.url`.
//!
//! The `apply_env_overrides` fn takes an env-source closure so it can be unit
//! tested with a `HashMap` without touching the process-wide env table.

use serde::Deserialize;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// Compile-time embedded default yaml. Serves two purposes:
/// 1. Written to disk on first boot (seed).
/// 2. Fallback yaml input when the file exists but is unreadable, or when the
///    seed itself failed (e.g. mount is `:ro`).
pub const DEFAULT_YAML: &str = include_str!("../config/config.default.yaml");

pub const DEFAULT_CONFIG_PATH: &str = "/etc/iam-service/config.yaml";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub jwt: JwtConfig,
    pub consul: ConsulConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    /// May be empty in the seeded yaml. Env `DATABASE_URL` fills it. Empty
    /// after merge → startup fails.
    #[serde(default)]
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    /// May be empty in the seeded yaml. Env `REDIS_URL` fills it. Empty after
    /// merge → startup fails.
    #[serde(default)]
    pub url: String,
    pub ttl_sec: u64,
    #[serde(default)]
    pub prefix: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub private_key_path: PathBuf,
    pub public_key_path: PathBuf,
    pub access_token_ttl_minutes: u64,
    pub refresh_token_ttl_days: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConsulConfig {
    pub host: String,
    pub port: u16,
    pub service_name: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loader
// ─────────────────────────────────────────────────────────────────────────────

pub fn load_config() -> anyhow::Result<Config> {
    let config_path =
        std::env::var("CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
    load_config_with_env(&config_path, |k| std::env::var(k).ok())
}

/// Testable entry point. Reads yaml from `config_path` (seeding it if needed),
/// applies env overrides via `env_lookup`, and validates the merged struct.
pub fn load_config_with_env<F>(config_path: &str, env_lookup: F) -> anyhow::Result<Config>
where
    F: Fn(&str) -> Option<String>,
{
    let path = Path::new(config_path);

    // ── Step 1: seed if absent ────────────────────────────────────────────
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match seed_if_absent(path) {
            Ok(true) => {
                tracing::info!(path = %config_path, "seeded config.yaml from embedded default")
            }
            Ok(false) => {} // peer won the race, fine
            Err(e) => tracing::warn!(
                error = %e,
                path = %config_path,
                "seed failed; will fall back to embedded default as yaml input"
            ),
        }
    }

    // ── Step 2: read yaml (fall back to embedded default on any read err) ─
    let yaml_content = fs::read_to_string(path).unwrap_or_else(|e| {
        tracing::warn!(
            error = %e,
            path = %config_path,
            "config file unreadable; using embedded default as yaml input"
        );
        DEFAULT_YAML.to_string()
    });
    let mut config: Config = serde_yaml::from_str(&yaml_content)?;

    // ── Step 3: env overrides ─────────────────────────────────────────────
    apply_env_overrides(&mut config, &env_lookup);

    // ── Step 4: merged-value validation ───────────────────────────────────
    validate_all_fields_present(&config)?;

    Ok(config)
}

/// Atomically create `path` and write `DEFAULT_YAML`. `create_new` guarantees
/// only one writer wins the race between iam-service and iam-outbox-publisher.
///
/// Returns `Ok(true)` on successful seed, `Ok(false)` if a peer already wrote
/// the file. Any other `io::Error` (`:ro` mount, permission denied, disk full)
/// is bubbled up and treated as a soft warning by the caller.
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

// ─────────────────────────────────────────────────────────────────────────────
// Env overrides
// ─────────────────────────────────────────────────────────────────────────────

/// Overwrites config fields from env. Malformed numeric values are logged and
/// the yaml value is kept, so a typo in one env var never turns into a hard
/// startup failure — the merged-value validator will catch true absence later.
pub fn apply_env_overrides<F>(cfg: &mut Config, env: &F)
where
    F: Fn(&str) -> Option<String>,
{
    // ── app ──
    if let Some(v) = env("SERVER_HOST") {
        cfg.app.host = v;
    }
    if let Some(v) = env("SERVER_PORT") {
        parse_or_warn("SERVER_PORT", &v, &mut cfg.app.port);
    }

    // ── database ──
    if let Some(v) = env("DATABASE_URL") {
        cfg.database.url = v;
    }
    if let Some(v) = env("DATABASE_MAX_CONNECTIONS") {
        parse_or_warn(
            "DATABASE_MAX_CONNECTIONS",
            &v,
            &mut cfg.database.max_connections,
        );
    }
    if let Some(v) = env("DATABASE_MIN_CONNECTIONS") {
        parse_or_warn(
            "DATABASE_MIN_CONNECTIONS",
            &v,
            &mut cfg.database.min_connections,
        );
    }

    // ── redis ──
    if let Some(v) = env("REDIS_URL") {
        cfg.redis.url = v;
    }
    if let Some(v) = env("REDIS_TTL_SEC") {
        parse_or_warn("REDIS_TTL_SEC", &v, &mut cfg.redis.ttl_sec);
    }
    if let Some(v) = env("REDIS_PREFIX") {
        cfg.redis.prefix = v;
    }

    // ── jwt ──
    if let Some(v) = env("JWT_PRIVATE_KEY_PATH") {
        cfg.jwt.private_key_path = PathBuf::from(v);
    }
    if let Some(v) = env("JWT_PUBLIC_KEY_PATH") {
        cfg.jwt.public_key_path = PathBuf::from(v);
    }
    if let Some(v) = env("JWT_ACCESS_TOKEN_TTL_MINUTES") {
        parse_or_warn(
            "JWT_ACCESS_TOKEN_TTL_MINUTES",
            &v,
            &mut cfg.jwt.access_token_ttl_minutes,
        );
    }
    if let Some(v) = env("JWT_REFRESH_TOKEN_TTL_DAYS") {
        parse_or_warn(
            "JWT_REFRESH_TOKEN_TTL_DAYS",
            &v,
            &mut cfg.jwt.refresh_token_ttl_days,
        );
    }

    // ── consul ──
    if let Some(v) = env("CONSUL_HOST") {
        cfg.consul.host = v;
    }
    if let Some(v) = env("CONSUL_PORT") {
        parse_or_warn("CONSUL_PORT", &v, &mut cfg.consul.port);
    }
    if let Some(v) = env("CONSUL_SERVICE_NAME") {
        cfg.consul.service_name = v;
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

/// Verifies every required field has a non-empty value AFTER env merge.
/// Explicit list of "must be non-empty" strings; other fields are checked
/// implicitly by serde (missing yaml key → deserialize error → fail).
pub fn validate_all_fields_present(cfg: &Config) -> anyhow::Result<()> {
    let mut missing: Vec<&'static str> = Vec::new();

    if cfg.app.name.trim().is_empty() {
        missing.push("app.name (set via yaml)");
    }
    if cfg.app.host.trim().is_empty() {
        missing.push("app.host (SERVER_HOST or yaml)");
    }
    if cfg.database.url.trim().is_empty() {
        missing.push("database.url (DATABASE_URL or yaml)");
    }
    if cfg.redis.url.trim().is_empty() {
        missing.push("redis.url (REDIS_URL or yaml)");
    }
    if cfg.jwt.private_key_path.as_os_str().is_empty() {
        missing.push("jwt.private_key_path (JWT_PRIVATE_KEY_PATH or yaml)");
    }
    if cfg.jwt.public_key_path.as_os_str().is_empty() {
        missing.push("jwt.public_key_path (JWT_PUBLIC_KEY_PATH or yaml)");
    }
    if cfg.consul.host.trim().is_empty() {
        missing.push("consul.host (CONSUL_HOST or yaml)");
    }
    if cfg.consul.service_name.trim().is_empty() {
        missing.push("consul.service_name (CONSUL_SERVICE_NAME or yaml)");
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "iam-service config missing required fields after env merge: {}",
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
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    /// Anti-drift: the embedded default yaml must deserialize into Config
    /// (with url fields empty; env is expected to fill them at runtime).
    #[test]
    fn embedded_default_yaml_deserializes() {
        let cfg: Config = serde_yaml::from_str(DEFAULT_YAML).expect("DEFAULT_YAML parses");
        assert_eq!(cfg.app.name, "iam-service");
        assert_eq!(cfg.app.port, 8081);
        assert!(
            cfg.database.url.is_empty(),
            "database.url is a placeholder in default"
        );
        assert!(
            cfg.redis.url.is_empty(),
            "redis.url is a placeholder in default"
        );
        assert_eq!(cfg.jwt.access_token_ttl_minutes, 15);
    }

    #[test]
    fn apply_env_overrides_string_fields() {
        let mut cfg: Config = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let env = env_from(&[
            ("DATABASE_URL", "mysql://u:p@h:3306/db"),
            ("REDIS_URL", "redis://h:6379/0"),
            ("REDIS_PREFIX", "test:"),
            ("SERVER_HOST", "127.0.0.1"),
            ("JWT_PRIVATE_KEY_PATH", "/tmp/priv.pem"),
            ("CONSUL_HOST", "consul.local"),
            ("CONSUL_SERVICE_NAME", "iam-svc-x"),
        ]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.database.url, "mysql://u:p@h:3306/db");
        assert_eq!(cfg.redis.url, "redis://h:6379/0");
        assert_eq!(cfg.redis.prefix, "test:");
        assert_eq!(cfg.app.host, "127.0.0.1");
        assert_eq!(cfg.jwt.private_key_path, PathBuf::from("/tmp/priv.pem"));
        assert_eq!(cfg.consul.host, "consul.local");
        assert_eq!(cfg.consul.service_name, "iam-svc-x");
    }

    #[test]
    fn apply_env_overrides_numeric_fields() {
        let mut cfg: Config = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let env = env_from(&[
            ("SERVER_PORT", "9999"),
            ("DATABASE_MAX_CONNECTIONS", "42"),
            ("DATABASE_MIN_CONNECTIONS", "7"),
            ("REDIS_TTL_SEC", "60"),
            ("JWT_ACCESS_TOKEN_TTL_MINUTES", "30"),
            ("JWT_REFRESH_TOKEN_TTL_DAYS", "14"),
            ("CONSUL_PORT", "18500"),
        ]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(cfg.app.port, 9999);
        assert_eq!(cfg.database.max_connections, 42);
        assert_eq!(cfg.database.min_connections, 7);
        assert_eq!(cfg.redis.ttl_sec, 60);
        assert_eq!(cfg.jwt.access_token_ttl_minutes, 30);
        assert_eq!(cfg.jwt.refresh_token_ttl_days, 14);
        assert_eq!(cfg.consul.port, 18500);
    }

    #[test]
    fn malformed_numeric_env_keeps_yaml_value() {
        let mut cfg: Config = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let yaml_port = cfg.app.port;
        let env = env_from(&[("SERVER_PORT", "not-a-number")]);
        apply_env_overrides(&mut cfg, &env);
        assert_eq!(
            cfg.app.port, yaml_port,
            "malformed env is ignored, yaml wins"
        );
    }

    #[test]
    fn empty_env_leaves_config_untouched() {
        let mut cfg: Config = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let snapshot = format!("{:?}", cfg);
        apply_env_overrides(&mut cfg, &env_from(&[]));
        assert_eq!(format!("{:?}", cfg), snapshot);
    }

    #[test]
    fn validate_rejects_missing_database_url() {
        let cfg: Config = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        // Default yaml leaves urls empty.
        let err = validate_all_fields_present(&cfg).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("database.url"), "err: {msg}");
        assert!(msg.contains("redis.url"), "err: {msg}");
    }

    #[test]
    fn validate_passes_when_urls_provided_via_env() {
        let mut cfg: Config = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        let env = env_from(&[("DATABASE_URL", "mysql://x"), ("REDIS_URL", "redis://x")]);
        apply_env_overrides(&mut cfg, &env);
        validate_all_fields_present(&cfg).expect("all fields present after env merge");
    }

    #[test]
    fn validate_rejects_whitespace_only_url() {
        let mut cfg: Config = serde_yaml::from_str(DEFAULT_YAML).unwrap();
        cfg.database.url = "   ".into();
        cfg.redis.url = "\t\n".into();
        let err = validate_all_fields_present(&cfg).unwrap_err();
        assert!(err.to_string().contains("database.url"));
    }

    #[test]
    fn seed_writes_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let wrote = seed_if_absent(&path).unwrap();
        assert!(wrote, "first call must write");
        let on_disk = fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, DEFAULT_YAML);
    }

    #[test]
    fn seed_returns_false_on_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "app:\n  name: preset\n").unwrap();
        let wrote = seed_if_absent(&path).unwrap();
        assert!(!wrote, "existing file must not be overwritten");
        // Content stays untouched.
        assert_eq!(fs::read_to_string(&path).unwrap(), "app:\n  name: preset\n");
    }

    /// Multiple threads racing to seed: exactly one succeeds, others get
    /// AlreadyExists → Ok(false). No panics, no partial writes.
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
        assert_eq!(wins, 1, "exactly one writer must win the race");
        assert_eq!(fs::read_to_string(path.as_path()).unwrap(), DEFAULT_YAML);
    }

    #[test]
    fn load_config_with_env_end_to_end_seed_then_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/dir/config.yaml"); // exercises create_dir_all
        let env = env_from(&[
            ("DATABASE_URL", "mysql://u:p@h/db"),
            ("REDIS_URL", "redis://r/0"),
            ("SERVER_PORT", "12345"),
        ]);
        let cfg = load_config_with_env(path.to_str().unwrap(), &env).expect("load should succeed");
        assert_eq!(cfg.app.port, 12345);
        assert_eq!(cfg.database.url, "mysql://u:p@h/db");
        assert_eq!(cfg.redis.url, "redis://r/0");
        assert!(path.exists(), "seed file must exist after load");
    }

    #[test]
    fn load_config_with_env_fails_when_urls_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let env = env_from(&[]);
        let err = load_config_with_env(path.to_str().unwrap(), &env).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("database.url"), "err: {msg}");
        assert!(msg.contains("redis.url"), "err: {msg}");
    }
}
