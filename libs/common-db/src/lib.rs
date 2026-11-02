pub mod dialect;
pub mod migration_state;
pub mod query;
pub mod scope;
pub mod sql;
#[cfg(feature = "test-kit")]
pub mod test_kit;

pub use scope::WithTenantScope;

/// 宏内需要引用 `sqlx::` 下的类型（`MySql` / `Postgres` / `Sqlite` / `Error` /
/// `QueryBuilder` …）。宏在**调用方 crate** 展开，调用方未必直接依赖 sqlx，
/// 因此统一经由 `$crate::sqlx::` 寻址——库侧的 feature 决定这些类型是否存在。
#[doc(hidden)]
pub use sqlx;

use dialect::DialectKind;
use digua_common_error::{AppError, AppResult};
#[cfg(feature = "mysql")]
use sqlx::mysql::MySql;
#[cfg(feature = "postgres")]
use sqlx::postgres::Postgres;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::Sqlite;
use sqlx::Pool;

/// 占位符风格 —— 供诊断 / 日志使用（业务层不应依赖它）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderStyle {
    /// 匿名占位符 `?`（MySQL / SQLite / 达梦 / MSSQL）。
    Question,
    /// 带编号占位符 `$1..$N`（PostgreSQL）。
    Numbered,
}

#[derive(Debug)]
pub enum DbPool {
    #[cfg(feature = "mysql")]
    Mysql(Pool<MySql>),
    #[cfg(feature = "postgres")]
    Postgres(Pool<Postgres>),
    #[cfg(feature = "sqlite")]
    Sqlite(Pool<Sqlite>),
}

impl DbPool {
    pub fn dialect(&self) -> DialectKind {
        match self {
            #[cfg(feature = "mysql")]
            Self::Mysql(_) => DialectKind::Mysql,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => DialectKind::Postgres,
            #[cfg(feature = "sqlite")]
            Self::Sqlite(_) => DialectKind::Sqlite,
        }
    }

    /// 占位符风格 —— PostgreSQL 是 `$N`，其余是 `?`。
    pub fn placeholder_style(&self) -> PlaceholderStyle {
        match self.dialect() {
            DialectKind::Postgres => PlaceholderStyle::Numbered,
            _ => PlaceholderStyle::Question,
        }
    }

    /// 健康探针（`common-health` 复用，避免各服务各写一遍）。
    pub async fn ping(&self) -> Result<(), sqlx::Error> {
        match self {
            #[cfg(feature = "mysql")]
            Self::Mysql(p) => {
                sqlx::query("SELECT 1").execute(p).await?;
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(p) => {
                sqlx::query("SELECT 1").execute(p).await?;
            }
            #[cfg(feature = "sqlite")]
            Self::Sqlite(p) => {
                sqlx::query("SELECT 1").execute(p).await?;
            }
        }
        Ok(())
    }

    /// 取 MySQL 连接池（非 MySQL 后端返回 `None`）。
    ///
    /// 用它替代 `let DbPool::Mysql(p) = pool;` —— 后者在新增变体后会变成
    /// 可反驳模式（E0005），编译期报错。
    #[cfg(feature = "mysql")]
    pub fn as_mysql(&self) -> Option<&sqlx::MySqlPool> {
        match self {
            Self::Mysql(p) => Some(p),
            _ => None,
        }
    }

    /// 取 PostgreSQL 连接池（非 PG 后端返回 `None`）。
    #[cfg(feature = "postgres")]
    pub fn as_postgres(&self) -> Option<&sqlx::PgPool> {
        match self {
            Self::Postgres(p) => Some(p),
            _ => None,
        }
    }

    /// 取 SQLite 连接池（非 SQLite 后端返回 `None`）。
    #[cfg(feature = "sqlite")]
    pub fn as_sqlite(&self) -> Option<&sqlx::SqlitePool> {
        match self {
            Self::Sqlite(p) => Some(p),
            _ => None,
        }
    }
}

pub async fn init_pool(database_url: &str) -> AppResult<DbPool> {
    init_pool_with(database_url, 10).await
}

/// 与 [`init_pool`] 相同，但允许调用方指定连接池上限。
///
/// 分方言设置 `max_connections`（SQLite 的并发写能力有限，沿用 5）。
pub async fn init_pool_with(database_url: &str, max_connections: u32) -> AppResult<DbPool> {
    let dialect = DialectKind::from_url(database_url);

    match dialect {
        #[cfg(feature = "mysql")]
        DialectKind::Mysql | DialectKind::Dameng => {
            let pool = sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(max_connections)
                .connect(database_url)
                .await
                .map_err(|e| AppError::internal("DB_CONNECT", e.to_string()))?;
            Ok(DbPool::Mysql(pool))
        }
        #[cfg(feature = "postgres")]
        DialectKind::Postgres => {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(max_connections)
                .after_connect(|conn, _meta| {
                    Box::pin(async move {
                        // Q2-bis：把会话时区锁死为 UTC，与基座层 `now_expr()` 的
                        // UTC 语义对齐；否则 `now()` / `date_trunc` 等会话敏感的
                        // 计算会随服务器本地时区漂移。
                        sqlx::query("SET TIME ZONE 'UTC'").execute(conn).await?;
                        Ok(())
                    })
                })
                .connect(database_url)
                .await
                .map_err(|e| AppError::internal("DB_CONNECT", e.to_string()))?;
            Ok(DbPool::Postgres(pool))
        }
        #[cfg(feature = "sqlite")]
        DialectKind::Sqlite => {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(max_connections.min(5))
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
        DialectKind::Postgres => "postgres",
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
        #[cfg(feature = "postgres")]
        DbPool::Postgres(_p) => Err(AppError::internal(
            "DB_MIGRATE",
            "runtime migration path not supported; use embedded Migrator",
        )),
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
    #[cfg(feature = "postgres")]
    pg: Option<sqlx::Transaction<'a, Postgres>>,
    #[cfg(feature = "sqlite")]
    sqlite: Option<sqlx::Transaction<'a, Sqlite>>,
    #[cfg(not(any(feature = "mysql", feature = "postgres", feature = "sqlite")))]
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
        #[cfg(feature = "postgres")]
        if let Some(tx) = self.pg.take() {
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
        #[cfg(feature = "postgres")]
        if let Some(tx) = self.pg.take() {
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
