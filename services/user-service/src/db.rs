use anyhow::{Context, Result};
use sqlx::mssql::{MssqlPool, MssqlPoolOptions};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::{DatabaseCfg, DbType};

#[derive(Clone)]
pub enum DbPool {
    Mysql(MySqlPool),
    Mssql(MssqlPool),
    Sqlite(SqlitePool),
    Dameng(Arc<Mutex<odbc_api::Connection<'static>>>),
}

impl DbPool {
    pub fn db_type(&self) -> DbType {
        match self {
            Self::Mysql(_) => DbType::Mysql,
            Self::Mssql(_) => DbType::Mssql,
            Self::Sqlite(_) => DbType::Sqlite,
            Self::Dameng(_) => DbType::Dameng,
        }
    }
}

pub async fn init_pool(cfg: &DatabaseCfg) -> Result<DbPool> {
    match cfg.db_type {
        DbType::Mysql => {
            let pool = MySqlPoolOptions::new()
                .max_connections(cfg.max_connections)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&cfg.url)
                .await
                .with_context(|| format!("connect mysql {}", cfg.url))?;
            Ok(DbPool::Mysql(pool))
        }
        DbType::Mssql => {
            let pool = MssqlPoolOptions::new()
                .max_connections(cfg.max_connections)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&cfg.url)
                .await
                .with_context(|| format!("connect mssql {}", cfg.url))?;
            Ok(DbPool::Mssql(pool))
        }
        DbType::Sqlite => {
            let pool = SqlitePoolOptions::new()
                .max_connections(cfg.max_connections.max(1))
                .acquire_timeout(Duration::from_secs(5))
                .connect(&cfg.url)
                .await
                .with_context(|| format!("connect sqlite {}", cfg.url))?;
            Ok(DbPool::Sqlite(pool))
        }
        DbType::Dameng => {
            use odbc_api::{ConnectionOptions, Environment};
            let env = Environment::new().context("create odbc env")?;
            let conn = env
                .connect_with_connection_string(&cfg.url, ConnectionOptions::default())
                .context("connect dameng via odbc")?;
            Ok(DbPool::Dameng(Arc::new(Mutex::new(conn))))
        }
    }
}
