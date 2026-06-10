use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Mysql,
    Mssql,
    Sqlite,
    Dameng,
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
pub struct RedisCfg {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthCfg {
    pub service_token_issuer: String,
    pub service_token_audience: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerCfg,
    pub database: DatabaseCfg,
    pub redis: RedisCfg,
    pub auth: AuthCfg,
}

pub fn load_config(path: &str) -> Result<Config> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read config {path}"))?;
    let cfg: Config = serde_yaml::from_str(&raw)
        .with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_db_type_mysql() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Mysql);
    }

    #[test]
    fn parse_db_type_mssql() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: mssql
  url: "mssql://u:p@localhost:1433/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Mssql);
    }

    #[test]
    fn parse_db_type_sqlite() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: sqlite
  url: "sqlite://./test.db"
  max_connections: 1
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Sqlite);
    }

    #[test]
    fn parse_db_type_dameng() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: dameng
  url: "Driver={DM8 ODBC DRIVER};Server=127.0.0.1;Port=5236;UID=SYSDBA;PWD=x;Schema=DMHR"
  max_connections: 1
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Dameng);
    }

    #[test]
    fn parse_db_type_missing() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        assert!(serde_yaml::from_str::<Config>(yaml).is_err());
    }

    #[test]
    fn parse_db_type_invalid() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: oracle
  url: "x"
  max_connections: 1
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        assert!(serde_yaml::from_str::<Config>(yaml).is_err());
    }
}
