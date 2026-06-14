pub mod dialect;
#[cfg(feature = "test-kit")]
pub mod test_kit;

use common_error::{AppError, AppResult};
use dialect::DialectKind;
use sqlx::{mysql::MySql, sqlite::Sqlite, mssql::Mssql, Pool};

#[derive(Debug)]
pub enum DbPool {
    Mysql(Pool<MySql>),
    Sqlite(Pool<Sqlite>),
    Mssql(Pool<Mssql>),
}

impl DbPool {
    pub fn dialect(&self) -> DialectKind {
        match self {
            Self::Mysql(_) => DialectKind::Mysql,
            Self::Sqlite(_) => DialectKind::Sqlite,
            Self::Mssql(_) => DialectKind::Mssql,
        }
    }
}

pub async fn init_pool(database_url: &str) -> AppResult<DbPool> {
    let dialect = DialectKind::from_url(database_url);
    
    match dialect {
        #[cfg(feature = "mysql")]
        DialectKind::Mysql | DialectKind::Dameng => {
            let pool = sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(10)
                .connect(database_url)
                .await
                .map_err(|e| AppError::internal("DB_CONNECT", e.to_string()))?;
            Ok(DbPool::Mysql(pool))
        }
        #[cfg(feature = "sqlite")]
        DialectKind::Sqlite => {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(5)
                .connect(database_url)
                .await
                .map_err(|e| AppError::internal("DB_CONNECT", e.to_string()))?;
            Ok(DbPool::Sqlite(pool))
        }
        #[cfg(feature = "mssql")]
        DialectKind::Mssql => {
            let pool = sqlx::mssql::MssqlPoolOptions::new()
                .max_connections(10)
                .connect(database_url)
                .await
                .map_err(|e| AppError::internal("DB_CONNECT", e.to_string()))?;
            Ok(DbPool::Mssql(pool))
        }
        #[cfg(not(any(feature = "mysql", feature = "sqlite", feature = "mssql")))]
        _ => Err(AppError::internal("DB_FEATURE", "no database feature enabled")),
    }
}

pub async fn run_migrations(pool: &DbPool, migrations_path: &str) -> AppResult<()> {
    let dialect = pool.dialect();
    let dialect_str = match dialect {
        DialectKind::Mysql => "mysql",
        DialectKind::Dameng => "mysql", // 达梦兼容 MySQL 迁移
        DialectKind::Sqlite => "sqlite",
        DialectKind::Mssql => "mssql",
    };
    
    let path = format!("{}/{}", migrations_path, dialect_str);
    
    match pool {
        #[cfg(feature = "mysql")]
        DbPool::Mysql(p) => {
            sqlx::migrate!(&path)
                .run(p)
                .await
                .map_err(|e| AppError::internal("DB_MIGRATE", e.to_string()))?;
        }
        #[cfg(feature = "sqlite")]
        DbPool::Sqlite(p) => {
            sqlx::migrate!(&path)
                .run(p)
                .await
                .map_err(|e| AppError::internal("DB_MIGRATE", e.to_string()))?;
        }
        #[cfg(feature = "mssql")]
        DbPool::Mssql(p) => {
            sqlx::migrate!(&path)
                .run(p)
                .await
                .map_err(|e| AppError::internal("DB_MIGRATE", e.to_string()))?;
        }
    }
    
    Ok(())
}

#[cfg(feature = "mysql")]
use sqlx::mysql::MySqlPool;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::SqlitePool;
#[cfg(feature = "mssql")]
use sqlx::mssql::MssqlPool;

pub struct Transaction<'a> {
    #[cfg(feature = "mysql")]
    mysql: Option<sqlx::Transaction<'a, MySql>>,
    #[cfg(feature = "sqlite")]
    sqlite: Option<sqlx::Transaction<'a, Sqlite>>,
    #[cfg(feature = "mssql")]
    mssql: Option<sqlx::Transaction<'a, Mssql>>,
}

impl<'a> Transaction<'a> {
    #[cfg(feature = "mysql")]
    pub async fn commit(mut self) -> AppResult<()> {
        if let Some(mut tx) = self.mysql.take() {
            tx.commit()
                .await
                .map_err(|e| AppError::internal("TX_COMMIT", e.to_string()))?;
        }
        Ok(())
    }
    
    #[cfg(feature = "sqlite")]
    pub async fn commit(mut self) -> AppResult<()> {
        if let Some(mut tx) = self.sqlite.take() {
            tx.commit()
                .await
                .map_err(|e| AppError::internal("TX_COMMIT", e.to_string()))?;
        }
        Ok(())
    }
    
    #[cfg(feature = "mssql")]
    pub async fn commit(mut self) -> AppResult<()> {
        if let Some(mut tx) = self.mssql.take() {
            tx.commit()
                .await
                .map_err(|e| AppError::internal("TX_COMMIT", e.to_string()))?;
        }
        Ok(())
    }
    
    #[cfg(feature = "mysql")]
    pub async fn rollback(mut self) -> AppResult<()> {
        if let Some(mut tx) = self.mysql.take() {
            tx.rollback()
                .await
                .map_err(|e| AppError::internal("TX_ROLLBACK", e.to_string()))?;
        }
        Ok(())
    }
    
    #[cfg(feature = "sqlite")]
    pub async fn rollback(mut self) -> AppResult<()> {
        if let Some(mut tx) = self.sqlite.take() {
            tx.rollback()
                .await
                .map_err(|e| AppError::internal("TX_ROLLBACK", e.to_string()))?;
        }
        Ok(())
    }
    
    #[cfg(feature = "mssql")]
    pub async fn rollback(mut self) -> AppResult<()> {
        if let Some(mut tx) = self.mssql.take() {
            tx.rollback()
                .await
                .map_err(|e| AppError::internal("TX_ROLLBACK", e.to_string()))?;
        }
        Ok(())
    }
}
