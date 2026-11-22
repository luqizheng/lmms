//! 事务性 outbox —— 方言无关（MySQL / PostgreSQL）。
//!
//! # 设计要点
//!
//! 1. **三条 SQL 的构造是纯函数**（[`enqueue_sql`] / [`claim_batch_sql`] /
//!    [`mark_published_sql`]），不依赖数据库即可单测（见 `tests/outbox_sql_test.rs`）。
//!    它们产出 **MySQL 风格（`?`）** 的 SQL，执行前统一经
//!    [`digua_common_db::sql::render`] 改写为目标方言（PG 得到 `$1..$N`）。
//! 2. **执行层按方言分派**，只有两处差异：
//!    - `enqueue`：MySQL `ON DUPLICATE KEY UPDATE id = id` ↔ PG `ON CONFLICT (event_id) DO NOTHING`
//!    - 占位符形态：由 `render` 统一处理
//! 3. `claim_batch` 的 `LIMIT ? FOR UPDATE SKIP LOCKED` 与
//!    `mark_published` 的 `CURRENT_TIMESTAMP(3)` **两方言通用**（PG 16 实测接受），
//!    不需要分支。
//!
//! # 事务参数：[`AnyTx`]
//!
//! `enqueue` 必须在**调用方的事务**里执行（outbox 与业务写入同事务，这是
//! transactional outbox 的全部意义）。事务类型是方言相关的，因此用 [`AnyTx`]
//! 枚举把两种事务包装起来。
//!
//! ```ignore
//! let mut tx = pool.begin().await?;
//! repo.enqueue(&mut AnyTx::Mysql(&mut tx), &mut env).await?;
//! tx.commit().await?;
//! ```

use async_trait::async_trait;
use digua_common_db::dialect::DialectKind;
use digua_common_db::sql::render;
use digua_common_db::DbPool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::envelope::Envelope;
use crate::error::OutboxError;
use crate::event::Event;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxRow {
    pub id: i64,
    pub event_id: Uuid,
    pub routing_key: String,
    pub envelope_json: serde_json::Value,
}

/// 方言相关的事务包装。
///
/// 两个生命周期参数：`'a` 是对事务的**可变借用**，`'c` 是事务自身的连接生命周期。
/// 二者不能合并为一个——`sqlx::Transaction<'c, DB>` 在 `'c` 上是不变的，
/// 强行取 `&'a mut Transaction<'a, DB>` 会把借用限制成「必须与事务同寿」，
/// 导致正常的局部事务无法传入。
pub enum AnyTx<'a, 'c> {
    #[cfg(feature = "mysql")]
    Mysql(&'a mut sqlx::Transaction<'c, sqlx::MySql>),
    #[cfg(feature = "postgres")]
    Postgres(&'a mut sqlx::Transaction<'c, sqlx::Postgres>),
}

impl AnyTx<'_, '_> {
    /// 本事务的方言。
    pub fn dialect(&self) -> DialectKind {
        match self {
            #[cfg(feature = "mysql")]
            Self::Mysql(_) => DialectKind::Mysql,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => DialectKind::Postgres,
            #[allow(unreachable_patterns)]
            _ => DialectKind::Mysql,
        }
    }
}

#[cfg(feature = "mysql")]
impl<'a, 'c> From<&'a mut sqlx::Transaction<'c, sqlx::MySql>> for AnyTx<'a, 'c> {
    fn from(tx: &'a mut sqlx::Transaction<'c, sqlx::MySql>) -> Self {
        Self::Mysql(tx)
    }
}

#[cfg(feature = "postgres")]
impl<'a, 'c> From<&'a mut sqlx::Transaction<'c, sqlx::Postgres>> for AnyTx<'a, 'c> {
    fn from(tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>) -> Self {
        Self::Postgres(tx)
    }
}

/// Internal helper: fill `env.trace_id` from `source()` only when caller left it None.
/// Extracted so tests can inject a fake source without an OTel runtime.
/// Caller-supplied trace_id (via `Envelope::with_trace_id(...)`) is always respected.
#[doc(hidden)]
pub fn fill_trace_id<P: Event, F: Fn() -> Option<String>>(env: &mut Envelope<P>, source: F) {
    if env.trace_id.is_none() {
        if let Some(tid) = source() {
            env.trace_id = Some(tid);
        }
    }
}

// ---------------------------------------------------------------------------
// 三条 SQL 的纯构造函数（MySQL 风格 `?`，执行前经 render 改写）
// ---------------------------------------------------------------------------

/// outbox 表名必须是安全标识符；它被**插入**进 SQL（不是 bind），
/// 因此只允许字母/数字/下划线。非法名字只可能来自调用方代码，不可能来自用户输入，
/// 故直接 panic（与 S1 之前的既有行为一致）。
fn assert_safe_table(table: &str) {
    assert!(
        !table.is_empty()
            && table
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "invalid outbox table name: {table:?}"
    );
}

/// `enqueue` 的 SQL（**唯一**需要按方言分叉的一条）。
///
/// - MySQL：`ON DUPLICATE KEY UPDATE id = id`（幂等忽略；既有行为，保持不变）
/// - PostgreSQL：`ON CONFLICT (event_id) DO NOTHING`
///   —— 用 `DO NOTHING` 而非 `DO UPDATE SET id = id`：语义就是幂等忽略，
///   后者会做一次无意义的空更新（额外 WAL / 行版本膨胀）。
pub fn enqueue_sql(table: &str, dialect: DialectKind) -> String {
    assert_safe_table(table);
    let on_conflict = match dialect {
        DialectKind::Postgres => "ON CONFLICT (event_id) DO NOTHING",
        _ => "ON DUPLICATE KEY UPDATE id = id",
    };
    format!(
        "INSERT INTO {table} (event_id, event_type, routing_key, envelope_json) \
         VALUES (?, ?, ?, ?) \
         {on_conflict}"
    )
}

/// `claim_batch` 的 SQL —— **两方言通用**。
///
/// `FOR UPDATE SKIP LOCKED` 让多个 publisher 副本抢到互不重叠的行；PG 与 MySQL 均支持。
/// `LIMIT ?` 在 PG 下由 [`render`] 改写为 `LIMIT $1`。
pub fn claim_batch_sql(table: &str) -> String {
    assert_safe_table(table);
    format!(
        "SELECT id, event_id, routing_key, envelope_json \
         FROM {table} \
         WHERE published_at IS NULL \
         ORDER BY id ASC \
         LIMIT ? \
         FOR UPDATE SKIP LOCKED"
    )
}

/// `mark_published` 的 SQL —— **两方言通用**。
///
/// `CURRENT_TIMESTAMP(3)` 在 PostgreSQL 16 上合法（实测），**不要**改写成 `now()`：
/// 前者是 SQL 标准写法，两方言都能跑，少一处分支。
///
/// `ids_len` 决定 `IN (?, ?, ...)` 的占位符个数；动态长度无法用静态 SQL 表达，
/// 因此这里拼接 `?`，再由 [`render`] 转成 `$1..$N`。
pub fn mark_published_sql(table: &str, ids_len: usize) -> String {
    assert_safe_table(table);
    let placeholders = vec!["?"; ids_len].join(",");
    format!("UPDATE {table} SET published_at = CURRENT_TIMESTAMP(3) WHERE id IN ({placeholders})")
}

/// 把 `?`-风格 SQL 渲染成目标方言，并校验 bind 个数。
///
/// 复用 S1 的 [`render`] + [`RewrittenQuery::expect_binds`]：
/// 「第 k 个 `?` ↔ 第 k 个 `.bind()`」的契约在**构造期**校验，
/// 与本仓 `db_*!` 宏走同一条防线（MySQL 侧同样校验）。
fn rendered(sql: &str, dialect: DialectKind, n_binds: usize) -> Result<String, OutboxError> {
    render(sql, dialect)
        .and_then(|rq| rq.expect_binds(n_binds))
        .map(|rq| rq.sql().to_string())
        .map_err(|e| OutboxError::Database(e.to_string()))
}

// ---------------------------------------------------------------------------
// Repo
// ---------------------------------------------------------------------------

#[async_trait]
pub trait OutboxRepo: Send + Sync {
    /// 在调用方事务内写入一条 outbox 记录（与业务写入同事务）。
    async fn enqueue<P: Event>(
        &self,
        tx: &mut AnyTx<'_, '_>,
        env: &mut Envelope<P>,
    ) -> Result<(), OutboxError>;

    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError>;

    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError>;
}

/// 方言无关的 outbox 仓库。
pub struct SqlOutboxRepo {
    pool: DbPool,
    table: String,
}

impl SqlOutboxRepo {
    /// 对默认 `iam_outbox_events` 表构造 repo。
    ///
    /// 参数是 `impl Into<DbPool>`，因此**既有的** `MySqlOutboxRepo::new(mysql_pool)`
    /// 调用点无需修改（`DbPool: From<MySqlPool>`）。
    pub fn new(pool: impl Into<DbPool>) -> Self {
        Self::with_table(pool, "iam_outbox_events")
    }

    /// 对显式表名构造 repo。
    pub fn with_table(pool: impl Into<DbPool>, table: impl Into<String>) -> Self {
        let table = table.into();
        assert_safe_table(&table);
        Self {
            pool: pool.into(),
            table,
        }
    }

    /// 底层连接池的方言。
    pub fn dialect(&self) -> DialectKind {
        self.pool.dialect()
    }

    fn unsupported(&self) -> OutboxError {
        OutboxError::Database(format!(
            "outbox repo does not support dialect {:?}",
            self.pool.dialect()
        ))
    }
}

#[async_trait]
impl OutboxRepo for SqlOutboxRepo {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut AnyTx<'_, '_>,
        env: &mut Envelope<P>,
    ) -> Result<(), OutboxError> {
        fill_trace_id(env, digua_common_telemetry::current_trace_id);
        let json = serde_json::to_value(&*env)?;
        let event_type = P::event_type();

        match tx {
            #[cfg(feature = "mysql")]
            AnyTx::Mysql(tx) => {
                let sql = rendered(
                    &enqueue_sql(&self.table, DialectKind::Mysql),
                    DialectKind::Mysql,
                    4,
                )?;
                sqlx::query(&sql)
                    .bind(env.event_id.to_string())
                    .bind(event_type)
                    .bind(event_type)
                    .bind(json)
                    .execute(&mut ***tx)
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
            }
            #[cfg(feature = "postgres")]
            AnyTx::Postgres(tx) => {
                let sql = rendered(
                    &enqueue_sql(&self.table, DialectKind::Postgres),
                    DialectKind::Postgres,
                    4,
                )?;
                sqlx::query(&sql)
                    .bind(env.event_id.to_string())
                    .bind(event_type)
                    .bind(event_type)
                    .bind(json)
                    .execute(&mut ***tx)
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
            }
            #[allow(unreachable_patterns)]
            _ => return Err(self.unsupported()),
        }
        Ok(())
    }

    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError> {
        let dialect = self.pool.dialect();
        let sql = rendered(&claim_batch_sql(&self.table), dialect, 1)?;

        // FOR UPDATE SKIP LOCKED 必须在事务内才能持有锁。
        let rows: Vec<(i64, String, String, serde_json::Value)> = match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(pool) => {
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
                let rows = sqlx::query_as(&sql)
                    .bind(limit as i64)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
                // Phase 4: at-least-once delivery. FOR UPDATE SKIP LOCKED releases
                // the lock on commit here, so the claim is advisory only —
                // mark_published is the real re-claim guard, backed by the
                // consumer's idempotency key.
                tx.commit()
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
                rows
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => {
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
                let rows = sqlx::query_as(&sql)
                    .bind(limit as i64)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
                rows
            }
            _ => return Err(self.unsupported()),
        };

        rows.into_iter()
            .map(|(id, eid, rk, env)| {
                Ok(OutboxRow {
                    id,
                    event_id: Uuid::parse_str(&eid)
                        .map_err(|e| OutboxError::Database(e.to_string()))?,
                    routing_key: rk,
                    envelope_json: env,
                })
            })
            .collect()
    }

    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError> {
        if ids.is_empty() {
            return Ok(());
        }
        let dialect = self.pool.dialect();
        // 动态 IN 列表：占位符由 mark_published_sql 拼 `?`，render 转 `$1..$N`。
        let sql = rendered(
            &mark_published_sql(&self.table, ids.len()),
            dialect,
            ids.len(),
        )?;

        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(pool) => {
                let mut q = sqlx::query(&sql);
                for id in ids {
                    q = q.bind(*id);
                }
                q.execute(pool)
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => {
                let mut q = sqlx::query(&sql);
                for id in ids {
                    q = q.bind(*id);
                }
                q.execute(pool)
                    .await
                    .map_err(|e| OutboxError::Database(e.to_string()))?;
            }
            _ => return Err(self.unsupported()),
        }
        Ok(())
    }
}

/// 历史名字，保留为别名以**不破坏既有调用点**（5 个服务 + 本 crate 的测试）。
///
/// 刻意**不加** `#[deprecated]`：本仓 clippy 门禁是
/// `cargo clippy --workspace --all-targets -- -D warnings`
/// （`.gitlab-ci.yml` / `.pre-commit-config.yaml`），
/// 而该别名有约 30 处使用点 —— 加 `#[deprecated]` 会让 clippy 门禁直接红。
/// 新代码请直接用 [`SqlOutboxRepo`]。
pub type MySqlOutboxRepo = SqlOutboxRepo;
