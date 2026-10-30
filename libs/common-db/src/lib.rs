pub mod dialect;
pub mod migration_state;
pub mod scope;
#[cfg(feature = "test-kit")]
pub mod test_kit;

#[cfg(feature = "mysql")]
pub use scope::WithTenantScope;

use dialect::DialectKind;
use digua_common_error::{AppError, AppResult};
#[cfg(feature = "mysql")]
use sqlx::mysql::MySql;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::Sqlite;
use sqlx::Pool;

#[derive(Debug)]
pub enum DbPool {
    #[cfg(feature = "mysql")]
    Mysql(Pool<MySql>),
    #[cfg(feature = "sqlite")]
    Sqlite(Pool<Sqlite>),
}

impl DbPool {
    pub fn dialect(&self) -> DialectKind {
        match self {
            #[cfg(feature = "mysql")]
            Self::Mysql(_) => DialectKind::Mysql,
            #[cfg(feature = "sqlite")]
            Self::Sqlite(_) => DialectKind::Sqlite,
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
        _ => Err(AppError::internal(
            "DB_FEATURE",
            "no database feature enabled for the requested dialect",
        )),
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

    let _ = format!("{}/{}", migrations_path, dialect_str);

    match pool {
        #[cfg(feature = "mysql")]
        DbPool::Mysql(_p) => {
            // 迁移路径需在编译期已知，运行期路径迁移请使用 sqlx::migrate::Migrator
            Err(AppError::internal(
                "DB_MIGRATE",
                "runtime migration path not supported; use embedded Migrator",
            ))
        }
        #[cfg(feature = "sqlite")]
        DbPool::Sqlite(_p) => Err(AppError::internal(
            "DB_MIGRATE",
            "runtime migration path not supported; use embedded Migrator",
        )),
    }
}

pub struct Transaction<'a> {
    #[cfg(feature = "mysql")]
    mysql: Option<sqlx::Transaction<'a, MySql>>,
    #[cfg(feature = "sqlite")]
    sqlite: Option<sqlx::Transaction<'a, Sqlite>>,
    #[cfg(not(any(feature = "mysql", feature = "sqlite")))]
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> Transaction<'a> {
    pub async fn commit(mut self) -> AppResult<()> {
        #[cfg(feature = "mysql")]
        if let Some(tx) = self.mysql.take() {
            tx.commit()
                .await
                .map_err(|e| AppError::internal("TX_COMMIT", e.to_string()))?;
        }
        #[cfg(feature = "sqlite")]
        if let Some(tx) = self.sqlite.take() {
            tx.commit()
                .await
                .map_err(|e| AppError::internal("TX_COMMIT", e.to_string()))?;
        }
        Ok(())
    }

    pub async fn rollback(mut self) -> AppResult<()> {
        #[cfg(feature = "mysql")]
        if let Some(tx) = self.mysql.take() {
            tx.rollback()
                .await
                .map_err(|e| AppError::internal("TX_ROLLBACK", e.to_string()))?;
        }
        #[cfg(feature = "sqlite")]
        if let Some(tx) = self.sqlite.take() {
            tx.rollback()
                .await
                .map_err(|e| AppError::internal("TX_ROLLBACK", e.to_string()))?;
        }
        Ok(())
    }
}
