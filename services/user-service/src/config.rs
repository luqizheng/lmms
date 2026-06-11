use anyhow::{Context, Result};
use serde::de;
use serde::Deserialize;
use std::fmt;
use std::fs;
use std::str::FromStr;
use std::time::Duration as StdDuration;

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
pub struct RedisCfg {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthCfg {
    pub service_token_issuer: String,
    pub service_token_audience: String,
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
            service_name: "user-service".into(),
            service_id_file: "/tmp/user-service-id".into(),
        }
    }
}

/// Parse duration from YAML: integer seconds ("300") or string with unit ("5m", "1m30s").
#[derive(Debug, Clone, Copy)]
pub struct Duration(StdDuration);

impl Duration {
    pub fn as_secs(&self) -> u64 {
        self.0.as_secs()
    }
}

impl FromStr for Duration {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if let Some(secs) = s.strip_suffix('s') {
            let rest = secs.strip_suffix('m').unwrap_or(secs);
            if rest.ends_with('m') {
                let n: u64 = rest
                    .trim_end_matches('m')
                    .parse::<u64>()
                    .map_err(|e| e.to_string())?;
                Ok(Duration(StdDuration::from_secs(n * 60)))
            } else {
                let n: u64 = rest.parse::<u64>().map_err(|e| e.to_string())?;
                Ok(Duration(StdDuration::from_secs(n)))
            }
        } else if let Some(mins) = s.strip_suffix('m') {
            let n: u64 = mins.parse::<u64>().map_err(|e| e.to_string())?;
            Ok(Duration(StdDuration::from_secs(n * 60)))
        } else {
            let n: u64 = s.parse::<u64>().map_err(|e| e.to_string())?;
            Ok(Duration(StdDuration::from_secs(n)))
        }
    }
}

impl<'de> de::Deserialize<'de> for Duration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = Duration;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a duration string like '5m' or '300'")
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Duration, E> {
                Ok(Duration(StdDuration::from_secs(v as u64)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Duration, E> {
                Ok(Duration(StdDuration::from_secs(v)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Duration, E> {
                v.parse().map_err(de::Error::custom)
            }
        }
        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Argon2Cfg {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for Argon2Cfg {
    fn default() -> Self {
        Self {
            memory_kib: 19456,
            iterations: 2,
            parallelism: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoginSecurityCfg {
    pub max_failures: u32,
    pub lockout_duration: Duration,
    pub counter_window: Duration,
    pub code_ttl: Duration,
    pub argon2: Argon2Cfg,
}

impl Default for LoginSecurityCfg {
    fn default() -> Self {
        Self {
            max_failures: 6,
            lockout_duration: Duration(StdDuration::from_secs(300)),
            counter_window: Duration(StdDuration::from_secs(900)),
            code_ttl: Duration(StdDuration::from_secs(300)),
            argon2: Argon2Cfg::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerCfg,
    pub database: DatabaseCfg,
    pub redis: RedisCfg,
    pub auth: AuthCfg,
    pub login_security: LoginSecurityCfg,
    pub consul: ConsulCfg,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerCfg {
                host: "0.0.0.0".into(),
                port: 8082,
            },
            database: DatabaseCfg {
                db_type: DbType::Mysql,
                url: "mysql://user:pass@localhost:3306/kudi".into(),
                max_connections: 16,
            },
            redis: RedisCfg {
                url: "redis://localhost:6379".into(),
            },
            auth: AuthCfg {
                service_token_issuer: "token-service".into(),
                service_token_audience: "user-service".into(),
            },
            login_security: LoginSecurityCfg::default(),
            consul: ConsulCfg::default(),
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
    fn parse_db_type_default_when_missing() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Mysql);
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

    #[test]
    fn login_security_default_applied() {
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
        assert_eq!(cfg.login_security.max_failures, 6);
        assert_eq!(cfg.login_security.lockout_duration.as_secs(), 300);
        assert_eq!(cfg.login_security.counter_window.as_secs(), 900);
        assert_eq!(cfg.login_security.code_ttl.as_secs(), 300);
        assert_eq!(cfg.login_security.argon2.memory_kib, 19456);
    }

    #[test]
    fn login_security_override() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
login_security:
  max_failures: 3
  lockout_duration: 1m
  counter_window: 2m
  code_ttl: 1m
  argon2: { memory_kib: 12288, iterations: 1, parallelism: 1 }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.login_security.max_failures, 3);
        assert_eq!(cfg.login_security.lockout_duration.as_secs(), 60);
        assert_eq!(cfg.login_security.counter_window.as_secs(), 120);
        assert_eq!(cfg.login_security.argon2.memory_kib, 12288);
    }

    #[test]
    fn duration_parsing() {
        assert_eq!("5m".parse::<Duration>().unwrap().as_secs(), 300);
        assert_eq!("1m".parse::<Duration>().unwrap().as_secs(), 60);
        assert_eq!("300".parse::<Duration>().unwrap().as_secs(), 300);
        assert_eq!("60".parse::<Duration>().unwrap().as_secs(), 60);
    }
}
