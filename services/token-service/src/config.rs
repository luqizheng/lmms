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
        Self {
            server: ServerCfg {
                host: "0.0.0.0".into(),
                port: 4000,
            },
            database: DatabaseCfg {
                db_type: DbType::Mysql,
                url: "mysql://token_service:token_pwd@localhost:3306/token_service".into(),
                max_connections: 16,
            },
            consul: ConsulCfg::default(),
            jwt_private_key_path: "keys/private.pem".into(),
            jwt_public_key_path: "keys/public.pem".into(),
            issuer: "superpowers-token-service".into(),
            default_access_ttl_sec: 3600,
            default_refresh_ttl_sec: 2592000,
            default_resource_ttl_sec: 300,
            user_service_url: "http://user-service:8080".into(),
        }
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let raw = fs::read_to_string(path).with_context(|| format!("read config {path}"))?;
    let cfg: Config = serde_yaml::from_str(&raw).with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
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
}
