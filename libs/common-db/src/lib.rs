pub mod dialect;
#[cfg(feature = "test-kit")]
pub mod test_kit;

use common_error::{AppError, AppResult};
use dialect::DialectKind;
use sqlx::{mysql::MySql, sqlite::Sqlite, Pool};

#[derive(Debug)]
pub enum DbPool {
    Mysql(Pool<MySql>),
    Sqlite(Pool<Sqlite>),
}

impl DbPool {
    pub fn dialect(&self) -> DialectKind {
        match self {
            Self::Mysql(_) => DialectKind::Mysql,
            Self::Sqlite(_) => DialectKind::Sqlite,
        }
    }
}

pub async fn init_pool(database_url: &str) -> AppResult<DbPool> {
    let dialect = DialectKind::from_url(database_url);
    
    match dialect {
        #[cfg(feature = "mysql")]
        DialectKind::Mysql | DialectKind::Dameng => {
            let pool = MySqlPool::connect(database_url)
                .await
                .map_err(|e| AppError::internal("DB_CONNECT", e.to_string()))?;
            Ok(DbPool::Mysql(pool))
        }
        #[cfg(feature = "sqlite")]
        DialectKind::Sqlite => {
            let pool = SqlitePool::connect(database_url)
                .await
                .map_err(|e| AppError::internal("DB_CONNECT", e.to_string()))?;
            Ok(DbPool::Sqlite(pool))
        }
        #[cfg(not(any(feature = "mysql", feature = "sqlite")))]
        _ => Err(AppError::internal("DB_FEATURE", "no database feature enabled")),
    }
}

pub async fn run_migrations(pool: &DbPool, migrations_path: &str) -> AppResult<()> {
    let dialect = pool.dialect();
    let dialect_str = match dialect {
        DialectKind::Mysql => "mysql",
        DialectKind::Dameng => "dameng",
        DialectKind::Sqlite => "sqlite",
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
    }
    
    Ok(())
}

pub struct Transaction<'a> {
    #[cfg(feature = "mysql")]
    mysql: Option<sqlx::Transaction<'a, MySql>>,
    #[cfg(feature = "sqlite")]
    sqlite: Option<sqlx::Transaction<'a, Sqlite>>,
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
}