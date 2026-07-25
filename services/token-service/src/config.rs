use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

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
        // empty and are enforced by `validate_all_fields_present` after the
        // Loader has merged env + yaml.
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

pub fn load_config(path: &str) -> Result<Config> {
    let raw = fs::read_to_string(path).with_context(|| format!("read config {path}"))?;
    let cfg: Config = serde_yaml::from_str(&raw).with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
}

/// Verifies every deployment-required field has a non-empty value AFTER the
/// Loader has merged env + yaml + defaults. Explicit list — non-sensitive
/// operational knobs (server.host/port, consul.*, ttl_sec) still fall back
/// to `Default`. See services/token-service/AGENTS.md for env var names.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
