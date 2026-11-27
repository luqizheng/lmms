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
    /// 匿名占位符 `?`（MySQL / SQLite）。
    Question,
    /// 带编号占位符 `$1..$N`（PostgreSQL）。
    Numbered,
}

/// 方言无关的连接池句柄。
///
/// `Clone` 只是克隆内部 `sqlx::Pool`（本身是 Arc 引用计数句柄），**不复制连接**，
/// 因此可以放心放进 `axum::Extension` / `AppState`。
#[derive(Debug, Clone)]
pub enum DbPool {
    #[cfg(feature = "mysql")]
    Mysql(Pool<MySql>),
    #[cfg(feature = "postgres")]
    Postgres(Pool<Postgres>),
    #[cfg(feature = "sqlite")]
    Sqlite(Pool<Sqlite>),
}

/// 由具体连接池直接构造 `DbPool`。
///
/// 这使得**已有的** `Foo::new(mysql_pool)` 调用点在 `new` 改成接受
/// `impl Into<DbPool>` 后**无需修改**——向后兼容的关键。
#[cfg(feature = "mysql")]
impl From<Pool<MySql>> for DbPool {
    fn from(pool: Pool<MySql>) -> Self {
        Self::Mysql(pool)
    }
}

#[cfg(feature = "postgres")]
impl From<Pool<Postgres>> for DbPool {
    fn from(pool: Pool<Postgres>) -> Self {
        Self::Postgres(pool)
    }
}

#[cfg(feature = "sqlite")]
impl From<Pool<Sqlite>> for DbPool {
    fn from(pool: Pool<Sqlite>) -> Self {
        Self::Sqlite(pool)
    }
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

    /// 开启一个方言感知事务（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-02）。
    ///
    /// 事务内的语句用 `db_tx_execute!` / `db_tx_fetch_optional!` /
    /// `db_tx_fetch_scalar!` / `db_tx_fetch_all!` 宏执行——同一份 SQL 源码
    /// 在 MySQL（`?`）与 PostgreSQL（`$N`）上按方言渲染，占位符改写与
    /// bind 计数校验与池级 `db_*!` 宏完全一致。
    ///
    /// 隔离级别用各后端默认（MySQL InnoDB `REPEATABLE READ` / PG
    /// `READ COMMITTED`）。需要行级串行化的读请加 `FOR UPDATE`
    ///（双方言语法一致），把先查后改的 TOCTOU 窗口交给行锁闭合。
    pub async fn begin(&self) -> Result<Transaction<'_>, sqlx::Error> {
        match self {
            #[cfg(feature = "mysql")]
            Self::Mysql(p) => Ok(Transaction::Mysql(p.begin().await?)),
            #[cfg(feature = "postgres")]
            Self::Postgres(p) => Ok(Transaction::Postgres(p.begin().await?)),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(p) => Ok(Transaction::Sqlite(p.begin().await?)),
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
    // 未知 scheme 明确报错（DB_URL_SCHEME），绝不静默回退到 MySQL。
    let dialect = DialectKind::from_url(database_url).ok_or_else(|| {
        AppError::internal(
            "DB_URL_SCHEME",
            "unsupported database URL scheme; supported schemes are mysql, mariadb, postgres, postgresql, sqlite",
        )
    })?;

    match dialect {
        #[cfg(feature = "mysql")]
        DialectKind::Mysql => {
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
        DialectKind::Sqlite => "sqlite",
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

/// 方言感知事务句柄（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-02 激活）。
///
/// 由 [`DbPool::begin`] 构造；事务内语句用 `db_tx_*!` 宏族执行（宏把
/// `&mut Transaction` 分派到对应后端，executor 为 `&mut **t`，即
/// `&mut DB::Connection`）。**drop 未提交的事务即回滚**（sqlx 语义），
/// 但成功路径应显式 [`Transaction::commit`]，失败路径显式
/// [`Transaction::rollback`] 以让错误可见。
///
/// 错误类型与 `db_*!` 宏族一致（`sqlx::Error`），调用方统一处理。
pub enum Transaction<'a> {
    #[cfg(feature = "mysql")]
    Mysql(sqlx::Transaction<'a, MySql>),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::Transaction<'a, Postgres>),
    #[cfg(feature = "sqlite")]
    Sqlite(sqlx::Transaction<'a, Sqlite>),
}

impl Transaction<'_> {
    /// 提交事务。任一后端 commit 失败 → `Err`（调用方必须视为操作失败，
    /// 不得向客户端确认成功）。
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        match self {
            #[cfg(feature = "mysql")]
            Self::Mysql(t) => t.commit().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(t) => t.commit().await,
            #[cfg(feature = "sqlite")]
            Self::Sqlite(t) => t.commit().await,
        }
    }

    /// 显式回滚。drop 未提交事务同样回滚（安全网），但失败路径应显式
    /// 调用本方法，让回滚错误进入调用方错误处理。
    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        match self {
            #[cfg(feature = "mysql")]
            Self::Mysql(t) => t.rollback().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(t) => t.rollback().await,
            #[cfg(feature = "sqlite")]
            Self::Sqlite(t) => t.rollback().await,
        }
    }
}
