//! notification-service 的方言无关 DB 层（MySQL / PostgreSQL）。
//!
//! 迁移脚本按方言选择（`migrations/mysql/` vs `migrations/postgres/`），
//! marker 探测统一走 `digua_common_db::migration_state::probe_marker`
//! （内部对 PG 用 `current_schema()`、MySQL 用 `DATABASE()`）。
//!
//! 与 `config-service/src/db.rs` / `token-service/src/db.rs` 保持同一套结构：
//! MIGRATIONS 数组 + `script_for` 方言选择 + `apply_script` 剥注释逐条执行。
//!
//! 事务方法（create_announcement / publish_announcement / apply_* /
//! replace_projection）沿用 `prue_backend/src/service/menu_config_svc.rs`
//! 的显式双臂样板：基座层没有跨方言事务抽象（`sqlx::Transaction<DB>` 的
//! `DB` 是具体类型），因此每个臂各自 `begin()` / `commit()`，臂内用原生
//! `sqlx::query`（`db_*!` 宏只吃 `DbPool`，不吃事务连接）。
//!
//! ⚠️ 执行器是「先按 ASCII 分号切分、再剥离 `--` 注释行」，
//! 因此**迁移文件的注释里不能出现 ASCII 分号**。

use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::{DateTime, Utc};
use digua_common_db::dialect::DialectKind;
use digua_common_db::WithTenantScope;
use digua_common_db::{db_execute, db_fetch_all, db_fetch_optional, migration_state, DbPool};
use digua_common_event::events::NotificationAnnouncementPublished;
use digua_common_event::{AnyTx, Envelope, MySqlOutboxRepo, OutboxRepo};
use digua_jwt_ctx::TenantScope;
use sqlx::QueryBuilder;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{info, warn};

const V001_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const V001_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V001__init.sql");
const V002_SQL_MYSQL: &str = include_str!("../migrations/mysql/V002__ntf_tenant.sql");
const V002_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V002__ntf_tenant.sql");
const V003_SQL_MYSQL: &str =
    include_str!("../migrations/mysql/V003__ntf_membership_projection.sql");
const V003_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V003__ntf_membership_projection.sql");
const V004_SQL_MYSQL: &str = include_str!("../migrations/mysql/V004__align_bigint_signed.sql");
const V004_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V004__align_bigint_signed.sql");

/// V001 的 marker 表：它创建的第一张业务表（与 `migration_state` 端点自描述一致）。
const SCHEMA_MARKER_TABLE: &str = "ntf_announcements";
/// V002 / V003 / V004 是 ALTER / 建投影表 / 去 UNSIGNED 型迁移，各自有专用哨兵表
/// （与 MySQL 侧同名）。
const V002_MARKER_TABLE: &str = "ntf_migration_v002_applied";
const V003_MARKER_TABLE: &str = "ntf_migration_v003_applied";
const V004_MARKER_TABLE: &str = "ntf_migration_v004_applied";

/// 单个迁移：版本号 + 双方言脚本 + marker table。
pub struct Migration {
    /// 版本号，`V001` / `V002` / `V003`。
    pub version: &'static str,
    pub name: &'static str,
    /// 判定该迁移是否已应用的 marker table（两端共用同名表）。
    pub marker_table: &'static str,
    pub sql_mysql: &'static str,
    pub sql_postgres: &'static str,
}

impl Migration {
    /// 按方言取脚本。
    pub fn sql_for(&self, dialect: DialectKind) -> &'static str {
        match dialect {
            DialectKind::Postgres => self.sql_postgres,
            _ => self.sql_mysql,
        }
    }
}

/// 按序执行的迁移列表。新增迁移只能追加到末尾。
///
/// marker 规则：V001 用它建的第一张业务表 `ntf_announcements`；
/// V002（ALTER）/ V003（建投影表）用专用哨兵表，由 `ensure_schema`
/// 在脚本执行后创建（PG 侧时间列为 TIMESTAMPTZ）。
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "V001",
        name: "init",
        marker_table: SCHEMA_MARKER_TABLE,
        sql_mysql: V001_SQL_MYSQL,
        sql_postgres: V001_SQL_POSTGRES,
    },
    Migration {
        version: "V002",
        name: "ntf_tenant",
        marker_table: V002_MARKER_TABLE,
        sql_mysql: V002_SQL_MYSQL,
        sql_postgres: V002_SQL_POSTGRES,
    },
    Migration {
        version: "V003",
        name: "ntf_membership_projection",
        marker_table: V003_MARKER_TABLE,
        sql_mysql: V003_SQL_MYSQL,
        sql_postgres: V003_SQL_POSTGRES,
    },
    Migration {
        version: "V004",
        name: "align_bigint_signed",
        marker_table: V004_MARKER_TABLE,
        sql_mysql: V004_SQL_MYSQL,
        sql_postgres: V004_SQL_POSTGRES,
    },
];

#[derive(Clone)]
pub struct Db {
    pub pool: DbPool,
    pub outbox_repo: Arc<MySqlOutboxRepo>,
}

impl Db {
    /// 按 URL 判定方言并建池（PG 分支由 `init_pool_with` 锁定会话时区为 UTC）。
    pub async fn connect(url: &str, max: u32) -> anyhow::Result<Self> {
        let pool = digua_common_db::init_pool_with(url, max)
            .await
            .map_err(|e| anyhow::anyhow!("DB_CONNECT: {e}"))?;
        let outbox_repo = Arc::new(MySqlOutboxRepo::with_table(
            pool.clone(),
            "ntf_outbox_events",
        ));
        Ok(Self { pool, outbox_repo })
    }

    pub async fn ensure_schema(&self) -> anyhow::Result<()> {
        let dialect = self.pool.dialect();

        // V001：marker 是它自己建的第一张业务表 ntf_announcements，无需哨兵 DDL。
        if !table_exists(&self.pool, SCHEMA_MARKER_TABLE).await? {
            info!(
                marker = SCHEMA_MARKER_TABLE,
                dialect = ?dialect,
                "schema missing, applying V001"
            );
            apply_script(
                &self.pool,
                script_for(dialect, V001_SQL_MYSQL, V001_SQL_POSTGRES),
                "V001",
            )
            .await?;
            info!("V001 applied");
        } else {
            info!(
                marker = SCHEMA_MARKER_TABLE,
                "schema already present, skip V001"
            );
        }

        // V002：ALTER 加 tenant_id，哨兵表判定（存量库不能拿 V001 的 marker 复用）。
        if !table_exists(&self.pool, V002_MARKER_TABLE).await? {
            info!(marker = V002_MARKER_TABLE, "applying V002");
            apply_script(
                &self.pool,
                script_for(dialect, V002_SQL_MYSQL, V002_SQL_POSTGRES),
                "V002",
            )
            .await?;
            db_execute!(&self.pool, marker_ddl(dialect, V002_MARKER_TABLE))?;
            info!("V002 applied");
        } else {
            info!(marker = V002_MARKER_TABLE, "V002 already applied, skip");
        }

        // V003：本地 user membership 投影表，哨兵表判定同 V002。
        if !table_exists(&self.pool, V003_MARKER_TABLE).await? {
            info!(marker = V003_MARKER_TABLE, "applying V003");
            apply_script(
                &self.pool,
                script_for(dialect, V003_SQL_MYSQL, V003_SQL_POSTGRES),
                "V003",
            )
            .await?;
            db_execute!(&self.pool, marker_ddl(dialect, V003_MARKER_TABLE))?;
            info!("V003 applied");
        } else {
            info!(marker = V003_MARKER_TABLE, "V003 already applied, skip");
        }

        // V004：MySQL 侧把 ntf_announcements 的 id / tenant_id 改为**有符号** BIGINT
        // （PG 侧建表时即无 UNSIGNED，同版本号为空操作文件）。两侧都建哨兵表，
        // 保证 marker 语义一致（PG 侧虽是空操作，仍应记录已应用）。
        if !table_exists(&self.pool, V004_MARKER_TABLE).await? {
            info!(marker = V004_MARKER_TABLE, "applying V004");
            apply_script(
                &self.pool,
                script_for(dialect, V004_SQL_MYSQL, V004_SQL_POSTGRES),
                "V004",
            )
            .await?;
            db_execute!(&self.pool, marker_ddl(dialect, V004_MARKER_TABLE))?;
            info!("V004 applied");
        } else {
            info!(marker = V004_MARKER_TABLE, "V004 already applied, skip");
        }

        Ok(())
    }

    /// 创建公告 + 写 recipients（同一事务）。
    ///
    /// 自增主键的取回方式两侧不同（归属表 #13）：MySQL `last_insert_id()`，
    /// PG `RETURNING id`（PG 无 last_insert_id()）→ 无法用一份 SQL 表达，
    /// 显式双臂（prue 样板）。
    pub async fn create_announcement(
        &self,
        a: &CreateAnnouncement,
        created_by: String,
    ) -> AppResult<i64> {
        let status = if a.publish_at <= Utc::now() {
            Status::Published
        } else {
            Status::Scheduled
        };
        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let res = sqlx::query(
                    "INSERT INTO ntf_announcements \
                     (title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at) \
                     VALUES (?,?,?,?,?,?,?,?,NOW())",
                )
                .bind(&a.title)
                .bind(&a.body)
                .bind(a.publish_at)
                .bind(a.expire_at)
                .bind(a.pinned) // bool（D-5：禁止 as i8 绑布尔列）
                .bind(a.popover)
                .bind(status)
                .bind(&created_by)
                .execute(&mut *tx)
                .await?;
                let id = res.last_insert_id() as i64;
                for s in &a.recipients {
                    sqlx::query(
                        "INSERT INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) VALUES (?,?,?)",
                    )
                    .bind(id)
                    .bind(s.scope_type)
                    .bind(&s.scope_id)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
                Ok(id)
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                // 事务内不走 db_*! 宏（宏只吃 DbPool），PG 臂直接写 $N 占位符（prue 样板）。
                let (id,): (i64,) = sqlx::query_as(
                    "INSERT INTO ntf_announcements \
                     (title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at) \
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW()) RETURNING id",
                )
                .bind(&a.title)
                .bind(&a.body)
                .bind(a.publish_at)
                .bind(a.expire_at)
                .bind(a.pinned)
                .bind(a.popover)
                .bind(status)
                .bind(&created_by)
                .fetch_one(&mut *tx)
                .await?;
                for s in &a.recipients {
                    sqlx::query(
                        "INSERT INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) VALUES ($1,$2,$3)",
                    )
                    .bind(id)
                    .bind(s.scope_type)
                    .bind(&s.scope_id)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
                Ok(id)
            }
            other => Err(AppError::from(anyhow::anyhow!(
                "notification-service: dialect {other:?} is not enabled for create_announcement"
            ))),
        }
    }

    pub async fn set_status(&self, id: i64, status: Status) -> AppResult<()> {
        db_execute!(
            &self.pool,
            "UPDATE ntf_announcements SET status=? WHERE id=?",
            status,
            id
        )?;
        Ok(())
    }

    pub async fn get(&self, id: i64) -> AppResult<Option<Announcement>> {
        Ok(db_fetch_optional!(
            &self.pool,
            Announcement,
            "SELECT id, title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at \
             FROM ntf_announcements WHERE id = ?",
            id
        )?)
    }

    pub async fn list_recipient_scopes(&self, ann_id: i64) -> AppResult<Vec<(ScopeType, String)>> {
        let rows: Vec<(String, String)> = db_fetch_all!(
            &self.pool,
            (String, String),
            "SELECT scope_type, scope_id FROM ntf_announcement_recipients WHERE announcement_id = ?",
            ann_id
        )?;
        Ok(rows
            .into_iter()
            .map(|(s, id)| match s.as_str() {
                "user" => (ScopeType::User, id),
                "role" => (ScopeType::Role, id),
                "dept" => (ScopeType::Dept, id),
                _ => (ScopeType::User, id),
            })
            .collect())
    }

    pub async fn due_scheduled(&self, now: DateTime<Utc>) -> AppResult<Vec<i64>> {
        let rows: Vec<(i64,)> = db_fetch_all!(
            &self.pool,
            (i64,),
            "SELECT id FROM ntf_announcements WHERE status='scheduled' AND publish_at <= ?",
            now
        )?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn list_for_user(&self, user_id: String) -> AppResult<Vec<Announcement>> {
        let mut items: Vec<Announcement> = db_fetch_all!(
            &self.pool,
            Announcement,
            "SELECT a.id, a.title, a.body, a.publish_at, a.expire_at, a.pinned, a.popover, \
                    a.status, a.created_by, a.created_at \
             FROM ntf_announcements a \
             JOIN ntf_announcement_deliveries d ON d.announcement_id = a.id \
             WHERE d.user_id=? AND a.status='published' \
               AND (a.expire_at IS NULL OR a.expire_at > NOW()) \
             ORDER BY a.pinned DESC, a.publish_at DESC",
            &user_id
        )?;

        let read_ids: Vec<(i64,)> = db_fetch_all!(
            &self.pool,
            (i64,),
            "SELECT announcement_id FROM ntf_announcement_reads WHERE user_id = ?",
            &user_id
        )?;
        let read_ids: HashSet<i64> = read_ids.into_iter().map(|(id,)| id).collect();
        for item in &mut items {
            item.read = read_ids.contains(&item.id);
        }

        Ok(items)
    }

    /// Marks every currently-delivered, unexpired announcement as read.
    pub async fn mark_all_read(&self, user_id: String) -> AppResult<u64> {
        const SQL_MYSQL: &str = "INSERT IGNORE INTO ntf_announcement_reads \
             (announcement_id, user_id, read_at) \
             SELECT a.id, ?, NOW() FROM ntf_announcements a \
             JOIN ntf_announcement_deliveries d ON d.announcement_id = a.id \
             WHERE d.user_id = ? AND a.status='published' \
               AND (a.expire_at IS NULL OR a.expire_at > NOW())";
        const SQL_PG: &str = "INSERT INTO ntf_announcement_reads \
             (announcement_id, user_id, read_at) \
             SELECT a.id, ?, NOW() FROM ntf_announcements a \
             JOIN ntf_announcement_deliveries d ON d.announcement_id = a.id \
             WHERE d.user_id = ? AND a.status='published' \
               AND (a.expire_at IS NULL OR a.expire_at > NOW()) \
             ON CONFLICT (announcement_id, user_id) DO NOTHING";

        let sql = match self.pool.dialect() {
            DialectKind::Postgres => SQL_PG,
            _ => SQL_MYSQL,
        };
        Ok(db_execute!(&self.pool, sql, &user_id, &user_id)?)
    }

    /// Admin view: every announcement in the tenant, including drafts,
    /// scheduled and revoked ones.
    pub async fn list_all_for_tenant(&self, tenant_id: i64) -> AppResult<Vec<Announcement>> {
        Ok(db_fetch_all!(
            &self.pool,
            Announcement,
            "SELECT id, title, body, publish_at, expire_at, pinned, popover, \
                    status, created_by, created_at \
             FROM ntf_announcements WHERE tenant_id = ? ORDER BY created_at DESC",
            tenant_id
        )?)
    }

    pub async fn insert_deliveries(&self, ann_id: i64, users: &HashSet<String>) -> AppResult<()> {
        if users.is_empty() {
            return Ok(());
        }
        let v: Vec<String> = users.iter().cloned().collect();
        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                insert_deliveries_batch_mysql(&mut tx, ann_id, &v).await?;
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                insert_deliveries_batch_pg(&mut tx, ann_id, &v).await?;
                tx.commit().await?;
            }
            other => {
                return Err(AppError::from(anyhow::anyhow!(
                    "notification-service: dialect {other:?} is not enabled for insert_deliveries"
                )))
            }
        }
        Ok(())
    }

    /// 标记已读（幂等：重复标记不算"新读"）。
    pub async fn mark_read(&self, ann_id: i64, user_id: String) -> AppResult<bool> {
        // 归属表 #8：INSERT IGNORE 仅 MySQL 有；PG 用 ON CONFLICT DO NOTHING。
        // 两条 SQL 只有这一处关键词不同，其余完全一致 → 两条 const 显式分支。
        const SQL_MYSQL: &str = "INSERT IGNORE INTO ntf_announcement_reads \
             (announcement_id, user_id, read_at) VALUES (?,?,NOW())";
        const SQL_PG: &str = "INSERT INTO ntf_announcement_reads \
             (announcement_id, user_id, read_at) VALUES (?,?,NOW()) \
             ON CONFLICT (announcement_id, user_id) DO NOTHING";

        let sql = match self.pool.dialect() {
            DialectKind::Postgres => SQL_PG,
            _ => SQL_MYSQL,
        };
        let res = db_execute!(&self.pool, sql, ann_id, user_id)?;
        Ok(res == 1) // db_execute! 直接返回 rows_affected: u64
    }

    /// 公告发布事务（spec §4.3）：tx1 锁行 → tx-out 本地投影 resolve scopes → tx2 幂等再锁 + deliveries + enqueue。
    ///
    /// ```text
    /// tx1
    ///   SELECT FOR UPDATE → status guard  (QueryBuilder + where_id + and_tenant)
    ///   COMMIT    （释放锁）
    /// tx-out
    ///   db.resolve_users_by_scopes(...) → HashSet<user_id>  (Phase 5B-fu3 本地 ntf_user_projection)
    /// tx2
    ///   SELECT status FOR UPDATE → 再次 guard（处理并发 publish）
    ///   batch INSERT IGNORE deliveries（Phase 4.3-fu1）
    ///   UPDATE status='published'
    ///   enqueue NotificationAnnouncementPublished → ntf_outbox_events
    ///   COMMIT
    /// ```
    /// 不在此处 hub.publish —— 推送统一由 announcement consumer 完成（避免双推）。
    /// 点查询 TenantScope 过滤（Phase 8.3-fu2）：CrossTenant 时 and_tenant no-op（运维跨租户支持）。
    /// Phase 5B-fu3：替换 `iam_client.expand_scopes` 为 `db.resolve_users_by_scopes`
    /// 本地投影查询 — 消除跨服务 HTTP 调用；投影由 startup bootstrap + 持续事件增量维护。
    pub async fn publish_announcement(
        &self,
        ann_id: i64,
        scope: &TenantScope,
    ) -> AppResult<Vec<String>> {
        let Some((title, pinned, popover)) = self.lock_announcement_header(ann_id, scope).await?
        else {
            return Ok(vec![]);
        };

        let scopes = self.list_recipient_scopes(ann_id).await?;
        let user_ids: HashSet<String> = if scopes.is_empty() {
            HashSet::new()
        } else {
            self.resolve_users_by_scopes(&scopes, scope).await?
        };

        let user_ids_vec: Vec<String> = user_ids.iter().cloned().collect();
        // tx2：二次守卫 + deliveries + status + outbox enqueue（同事务，原子生效）。
        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let mut qb =
                    QueryBuilder::<sqlx::MySql>::new("SELECT status FROM ntf_announcements");
                qb.where_id(ann_id);
                qb.and_tenant(scope);
                qb.push(" FOR UPDATE");
                let (status_again,): (String,) =
                    qb.build_query_as::<(String,)>().fetch_one(&mut *tx).await?;
                if status_again == "published" || status_again == "revoked" {
                    return Ok(user_ids_vec);
                }
                insert_deliveries_batch_mysql(&mut tx, ann_id, &user_ids_vec).await?;
                let mut qb = QueryBuilder::<sqlx::MySql>::new(
                    "UPDATE ntf_announcements SET status='published'",
                );
                qb.where_id(ann_id);
                qb.and_tenant(scope);
                qb.build().execute(&mut *tx).await?;
                let mut envelope = Envelope::new(
                    "notification-service",
                    NotificationAnnouncementPublished {
                        announcement_id: ann_id,
                        title,
                        pinned,
                        popover,
                        user_ids: serde_json::json!(user_ids),
                    },
                );
                self.outbox_repo
                    .clone()
                    .enqueue(&mut AnyTx::Mysql(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| AppError::from(anyhow::anyhow!("outbox enqueue: {e}")))?;
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let mut qb =
                    QueryBuilder::<sqlx::Postgres>::new("SELECT status FROM ntf_announcements");
                qb.where_id(ann_id);
                qb.and_tenant(scope);
                qb.push(" FOR UPDATE");
                let (status_again,): (String,) =
                    qb.build_query_as::<(String,)>().fetch_one(&mut *tx).await?;
                if status_again == "published" || status_again == "revoked" {
                    return Ok(user_ids_vec);
                }
                insert_deliveries_batch_pg(&mut tx, ann_id, &user_ids_vec).await?;
                let mut qb = QueryBuilder::<sqlx::Postgres>::new(
                    "UPDATE ntf_announcements SET status='published'",
                );
                qb.where_id(ann_id);
                qb.and_tenant(scope);
                qb.build().execute(&mut *tx).await?;
                let mut envelope = Envelope::new(
                    "notification-service",
                    NotificationAnnouncementPublished {
                        announcement_id: ann_id,
                        title,
                        pinned,
                        popover,
                        user_ids: serde_json::json!(user_ids),
                    },
                );
                // AnyTx 按事务的实际方言包装（AnyTx::Mysql/Postgres 是方言相关枚举）。
                self.outbox_repo
                    .clone()
                    .enqueue(&mut AnyTx::Postgres(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| AppError::from(anyhow::anyhow!("outbox enqueue: {e}")))?;
                tx.commit().await?;
            }
            other => {
                return Err(AppError::from(anyhow::anyhow!(
                "notification-service: dialect {other:?} is not enabled for publish_announcement"
            )))
            }
        }
        Ok(user_ids_vec)
    }

    /// tx1：`SELECT ... FOR UPDATE` 锁行并读 header；行不存在或已 published/revoked
    /// 时返回 `None`（调用方短路）。每臂各自 begin/commit（prue 双臂样板）。
    async fn lock_announcement_header(
        &self,
        ann_id: i64,
        scope: &TenantScope,
    ) -> AppResult<Option<(String, bool, bool)>> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::MySql>::new(
                    "SELECT title, status, pinned, popover FROM ntf_announcements",
                );
                qb.where_id(ann_id);
                qb.and_tenant(scope);
                qb.push(" FOR UPDATE");
                // MySQL TINYINT(1) 列按 bool 解码（D-5 实测可行）。
                let row: Option<(String, String, bool, bool)> =
                    qb.build_query_as().fetch_optional(&mut *tx).await?;
                tx.commit().await?;
                Ok(row.and_then(|(title, status, pinned, popover)| {
                    if status == "published" || status == "revoked" {
                        None
                    } else {
                        Some((title, pinned, popover))
                    }
                }))
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::Postgres>::new(
                    "SELECT title, status, pinned, popover FROM ntf_announcements",
                );
                qb.where_id(ann_id);
                qb.and_tenant(scope);
                qb.push(" FOR UPDATE");
                let row: Option<(String, String, bool, bool)> =
                    qb.build_query_as().fetch_optional(&mut *tx).await?;
                tx.commit().await?;
                Ok(row.and_then(|(title, status, pinned, popover)| {
                    if status == "published" || status == "revoked" {
                        None
                    } else {
                        Some((title, pinned, popover))
                    }
                }))
            }
            other => Err(AppError::from(anyhow::anyhow!(
                "notification-service: dialect {other:?} is not enabled for lock_announcement_header"
            ))),
        }
    }

    // ===== Phase 5B-fu3: membership projection =====

    /// Apply `iam.user.role.assigned` event.
    /// Out-of-order: ts < last_event_at → drop (idempotent replay safe).
    pub async fn apply_role_assigned(
        &self,
        user_id: &str,
        role_id: &str,
        tenant_id: i64,
        ts: i64,
    ) -> AppResult<()> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT last_event_at FROM ntf_user_projection WHERE user_id = ? FOR UPDATE",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                let current_ts = row.map(|(t,)| t).unwrap_or(0);
                if ts < current_ts {
                    return Ok(());
                }
                if row.is_some() {
                    sqlx::query(
                        "UPDATE ntf_user_projection \
                         SET role_ids = JSON_ARRAY_APPEND(role_ids, '$', ?), last_event_at = ? \
                         WHERE user_id = ?",
                    )
                    .bind(role_id)
                    .bind(ts)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                } else {
                    sqlx::query(
                        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
                         VALUES (?, ?, JSON_ARRAY(?), NULL, ?)",
                    )
                    .bind(user_id)
                    .bind(tenant_id)
                    .bind(role_id)
                    .bind(ts)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT last_event_at FROM ntf_user_projection WHERE user_id = $1 FOR UPDATE",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                let current_ts = row.map(|(t,)| t).unwrap_or(0);
                if ts < current_ts {
                    return Ok(());
                }
                if row.is_some() {
                    // jsonb 数组追加：`role_ids || to_jsonb(<text>)`（PG 无 JSON_ARRAY_APPEND）。
                    sqlx::query(
                        "UPDATE ntf_user_projection \
                         SET role_ids = role_ids || to_jsonb($1::text), last_event_at = $2 \
                         WHERE user_id = $3",
                    )
                    .bind(role_id)
                    .bind(ts)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                } else {
                    sqlx::query(
                        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
                         VALUES ($1, $2, jsonb_build_array($3), NULL, $4)",
                    )
                    .bind(user_id)
                    .bind(tenant_id)
                    .bind(role_id)
                    .bind(ts)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
            other => {
                return Err(AppError::from(anyhow::anyhow!(
                    "notification-service: dialect {other:?} is not enabled for apply_role_assigned"
                )))
            }
        }
        Ok(())
    }

    /// Apply `iam.user.role.unassigned` event.
    /// No-op if user row absent (consumer may receive unassign before assign
    /// during bootstrap grace window). Out-of-order dropped.
    pub async fn apply_role_unassigned(
        &self,
        user_id: &str,
        role_id: &str,
        _tenant_id: i64,
        ts: i64,
    ) -> AppResult<()> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT last_event_at FROM ntf_user_projection WHERE user_id = ? FOR UPDATE",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                let current_ts = row.map(|(t,)| t).unwrap_or(0);
                if ts < current_ts {
                    return Ok(());
                }
                if row.is_none() {
                    return Ok(());
                }
                // JSON_SEARCH returns the path (e.g. "$[2]") or NULL; only remove if present.
                sqlx::query(
                    "UPDATE ntf_user_projection \
                     SET role_ids = JSON_REMOVE(\
                          role_ids, \
                          JSON_UNQUOTE(JSON_SEARCH(role_ids, 'one', ?))\
                        ), \
                        last_event_at = ? \
                     WHERE user_id = ?",
                )
                .bind(role_id)
                .bind(ts)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT last_event_at FROM ntf_user_projection WHERE user_id = $1 FOR UPDATE",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                let current_ts = row.map(|(t,)| t).unwrap_or(0);
                if ts < current_ts {
                    return Ok(());
                }
                if row.is_none() {
                    return Ok(());
                }
                // jsonb 减 text（`role_ids - $1::text`）删除数组元素。
                // ⚠️ 语义差异（US-6 记录）：MySQL JSON_SEARCH('one') 只删**首个**匹配，
                // PG `-` 删除**全部**匹配。同一 role 对 user 只 assign 一次，业务可接受。
                sqlx::query(
                    "UPDATE ntf_user_projection \
                     SET role_ids = role_ids - $1::text, last_event_at = $2 \
                     WHERE user_id = $3",
                )
                .bind(role_id)
                .bind(ts)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
            other => {
                return Err(AppError::from(anyhow::anyhow!(
                "notification-service: dialect {other:?} is not enabled for apply_role_unassigned"
            )))
            }
        }
        Ok(())
    }

    /// Apply `iam.user.dept.changed` event. dept_id=None means user removed
    /// from any org_unit (dept_id set to NULL).
    pub async fn apply_dept_changed(
        &self,
        user_id: &str,
        dept_id: Option<&str>,
        tenant_id: i64,
        ts: i64,
    ) -> AppResult<()> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT last_event_at FROM ntf_user_projection WHERE user_id = ? FOR UPDATE",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                let current_ts = row.map(|(t,)| t).unwrap_or(0);
                if ts < current_ts {
                    return Ok(());
                }
                if row.is_some() {
                    sqlx::query(
                        "UPDATE ntf_user_projection SET dept_id = ?, last_event_at = ? WHERE user_id = ?",
                    )
                    .bind(dept_id)
                    .bind(ts)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                } else {
                    // No prior role events yet (rare: dept change before role assignment).
                    // Insert with empty role_ids and the new dept.
                    sqlx::query(
                        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
                         VALUES (?, ?, JSON_ARRAY(), ?, ?)",
                    )
                    .bind(user_id)
                    .bind(tenant_id)
                    .bind(dept_id)
                    .bind(ts)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT last_event_at FROM ntf_user_projection WHERE user_id = $1 FOR UPDATE",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                let current_ts = row.map(|(t,)| t).unwrap_or(0);
                if ts < current_ts {
                    return Ok(());
                }
                if row.is_some() {
                    sqlx::query(
                        "UPDATE ntf_user_projection SET dept_id = $1, last_event_at = $2 WHERE user_id = $3",
                    )
                    .bind(dept_id)
                    .bind(ts)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                } else {
                    // No prior role events yet (rare: dept change before role assignment).
                    // Insert with empty role_ids and the new dept.
                    sqlx::query(
                        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
                         VALUES ($1, $2, '[]'::jsonb, $3, $4)",
                    )
                    .bind(user_id)
                    .bind(tenant_id)
                    .bind(dept_id)
                    .bind(ts)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
            other => {
                return Err(AppError::from(anyhow::anyhow!(
                    "notification-service: dialect {other:?} is not enabled for apply_dept_changed"
                )))
            }
        }
        Ok(())
    }

    /// Replace projection for a tenant from bootstrap snapshot.
    /// Deletes prior rows then batch-inserts (10K user scale, single tx).
    /// `last_event_at` set to 0; subsequent consumer events with ts > 0 apply normally.
    pub async fn replace_projection(
        &self,
        tenant_id: i64,
        mappings: &HashMap<String, crate::models::UserMembership>,
    ) -> AppResult<()> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                sqlx::query("DELETE FROM ntf_user_projection WHERE tenant_id = ?")
                    .bind(tenant_id)
                    .execute(&mut *tx)
                    .await?;
                for (uid, m) in mappings {
                    let role_ids_json = serde_json::to_string(&m.role_ids)
                        .map_err(|e| AppError::from(anyhow::anyhow!("role_ids serialize: {e}")))?;
                    sqlx::query(
                        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
                         VALUES (?, ?, ?, ?, 0)",
                    )
                    .bind(uid)
                    .bind(tenant_id)
                    .bind(role_ids_json)
                    .bind(&m.dept_id)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                sqlx::query("DELETE FROM ntf_user_projection WHERE tenant_id = $1")
                    .bind(tenant_id)
                    .execute(&mut *tx)
                    .await?;
                for (uid, m) in mappings {
                    let role_ids_json = serde_json::to_string(&m.role_ids)
                        .map_err(|e| AppError::from(anyhow::anyhow!("role_ids serialize: {e}")))?;
                    // role_ids 是 JSONB 列，绑定的是 serde_json 文本（text 参数），
                    // PG 无 text→jsonb 隐式赋值 cast，必须显式 `$3::jsonb`。
                    sqlx::query(
                        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
                         VALUES ($1, $2, $3::jsonb, $4, 0)",
                    )
                    .bind(uid)
                    .bind(tenant_id)
                    .bind(role_ids_json)
                    .bind(&m.dept_id)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
            other => {
                return Err(AppError::from(anyhow::anyhow!(
                    "notification-service: dialect {other:?} is not enabled for replace_projection"
                )))
            }
        }
        Ok(())
    }

    /// Resolve user_ids from a set of scopes (user/role/dept) by querying the
    /// local projection. TenantScope::Single(n) → queries tenant n.
    /// TenantScope::CrossTenant → falls back to caller-provided mapping (kept
    /// for platform admin path; current callers always Single).
    pub async fn resolve_users_by_scopes(
        &self,
        scopes: &[(crate::models::ScopeType, String)],
        scope: &TenantScope,
    ) -> AppResult<HashSet<String>> {
        let tenant_id = scope.tenant_id().unwrap_or(1);
        let mut out = HashSet::new();
        for (t, id) in scopes {
            match t {
                crate::models::ScopeType::User => {
                    out.insert(id.clone());
                }
                crate::models::ScopeType::Role => {
                    // 归属表 #6 类：JSON 包含判断两侧语法不同（US-6 显式分支，
                    // 唯一允许"两条 SQL"的地方）：
                    //   MySQL: JSON_CONTAINS(role_ids, JSON_QUOTE(?))
                    //   PG:    role_ids @> to_jsonb(?::text) —— jsonb 顶层包含运算符
                    const SQL_MYSQL: &str = "SELECT user_id FROM ntf_user_projection \
                         WHERE tenant_id = ? \
                           AND JSON_CONTAINS(role_ids, JSON_QUOTE(?))";
                    const SQL_PG: &str = "SELECT user_id FROM ntf_user_projection \
                         WHERE tenant_id = ? \
                           AND role_ids @> to_jsonb(?::text)";
                    let sql = match self.pool.dialect() {
                        DialectKind::Postgres => SQL_PG,
                        _ => SQL_MYSQL,
                    };
                    let rows: Vec<(String,)> =
                        db_fetch_all!(&self.pool, (String,), sql, tenant_id, id)?;
                    out.extend(rows.into_iter().map(|(s,)| s));
                }
                crate::models::ScopeType::Dept => {
                    let rows: Vec<(String,)> = db_fetch_all!(
                        &self.pool,
                        (String,),
                        "SELECT user_id FROM ntf_user_projection \
                         WHERE tenant_id = ? AND dept_id = ?",
                        tenant_id,
                        id
                    )?;
                    out.extend(rows.into_iter().map(|(s,)| s));
                }
            }
        }
        Ok(out)
    }
}

/// 按 URL 判定方言取迁移脚本。
fn script_for(dialect: DialectKind, mysql: &'static str, postgres: &'static str) -> &'static str {
    match dialect {
        DialectKind::Postgres => postgres,
        _ => mysql,
    }
}

/// marker 哨兵表的 DDL（方言相关：PG 的时间列必须 TIMESTAMPTZ）。
fn marker_ddl(dialect: DialectKind, table: &str) -> String {
    let ts = match dialect {
        DialectKind::Postgres => "TIMESTAMPTZ",
        _ => "DATETIME",
    };
    format!(
        "CREATE TABLE IF NOT EXISTS {table} \
         (applied_at {ts} NOT NULL DEFAULT CURRENT_TIMESTAMP)"
    )
}

/// 逐条执行迁移脚本（与历史行为一致：先按 `;` 切分，再剥离注释行）。
async fn apply_script(pool: &DbPool, script: &str, label: &str) -> anyhow::Result<()> {
    for stmt in script.split(';') {
        let cleaned: String = stmt
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        let s = cleaned.trim();
        if s.is_empty() {
            continue;
        }
        let upper = s.to_ascii_uppercase();
        // MySQL 侧迁移文件头部可能有 CREATE DATABASE / USE（历史遗留），跳过。
        if upper.starts_with("CREATE DATABASE") || upper.starts_with("USE ") {
            continue;
        }
        if let Err(e) = db_execute!(pool, s) {
            warn!(statement = %first_line(s), error = %e, "{} statement failed", label);
            return Err(anyhow::anyhow!("apply {label} ({}): {e}", first_line(s)));
        }
    }
    Ok(())
}

async fn table_exists(pool: &DbPool, table: &str) -> anyhow::Result<bool> {
    // 统一走基座层入口：PG 用 current_schema()，MySQL 用 DATABASE()。
    migration_state::probe_marker(pool, table)
        .await
        .map_err(|e| anyhow::anyhow!("probe marker {table}: {e}"))
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty() && !l.trim().starts_with("--"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
}

/// 批量写 deliveries —— MySQL 臂：`INSERT IGNORE`（方言专属语法，归属表 #8）。
#[cfg(feature = "mysql")]
async fn insert_deliveries_batch_mysql(
    conn: &mut sqlx::MySqlConnection,
    ann_id: i64,
    users: &[String],
) -> AppResult<()> {
    if users.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::<sqlx::MySql>::new(
        "INSERT IGNORE INTO ntf_announcement_deliveries \
         (announcement_id, user_id, delivered_at) ",
    );
    qb.push_values(users.iter(), |mut b, uid| {
        b.push_bind(ann_id).push_bind(uid).push("NOW()");
    });
    qb.build().execute(conn).await?;
    Ok(())
}

/// 批量写 deliveries —— PG 臂：`ON CONFLICT DO NOTHING`（等价幂等）。
/// `QueryBuilder::<sqlx::Postgres>` 的 `push_bind` 自动产出 `$1..$N`。
#[cfg(feature = "postgres")]
async fn insert_deliveries_batch_pg(
    conn: &mut sqlx::PgConnection,
    ann_id: i64,
    users: &[String],
) -> AppResult<()> {
    if users.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "INSERT INTO ntf_announcement_deliveries \
         (announcement_id, user_id, delivered_at) ",
    );
    qb.push_values(users.iter(), |mut b, uid| {
        b.push_bind(ann_id).push_bind(uid).push("NOW()");
    });
    qb.push(" ON CONFLICT DO NOTHING");
    qb.build().execute(conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_sql_contains_all_tables() {
        for dialect in [DialectKind::Mysql, DialectKind::Postgres] {
            let upper = MIGRATIONS[0].sql_for(dialect).to_uppercase();
            for tbl in [
                "NTF_ANNOUNCEMENTS",
                "NTF_ANNOUNCEMENT_RECIPIENTS",
                "NTF_ANNOUNCEMENT_DELIVERIES",
                "NTF_ANNOUNCEMENT_READS",
                "NTF_TEMPLATES",
                "NTF_EVENT_IDEMPOTENCY",
                "NTF_OUTBOX_EVENTS",
            ] {
                assert!(
                    upper.contains(&format!("CREATE TABLE IF NOT EXISTS {tbl}")),
                    "missing CREATE TABLE for {tbl} in {dialect:?}"
                );
            }
        }
    }

    #[test]
    fn first_line_skips_comments_and_blanks() {
        assert_eq!(
            first_line("\n  -- hi\n  CREATE TABLE x (id INT);"),
            "CREATE TABLE x (id INT);"
        );
        assert_eq!(first_line("-- only comment"), "<empty>");
    }

    #[test]
    fn init_sql_seeds_welcome_template() {
        for dialect in [DialectKind::Mysql, DialectKind::Postgres] {
            let sql = MIGRATIONS[0].sql_for(dialect);
            assert!(sql.to_uppercase().contains("INSERT INTO NTF_TEMPLATES"));
            assert!(sql.contains("'welcome'"), "{dialect:?} must seed welcome");
        }
    }

    #[test]
    fn init_sql_outbox_columns() {
        let upper = MIGRATIONS[0].sql_for(DialectKind::Mysql).to_uppercase();
        for col in [
            "EVENT_ID",
            "EVENT_TYPE",
            "ROUTING_KEY",
            "ENVELOPE_JSON",
            "PUBLISHED_AT",
        ] {
            assert!(upper.contains(col), "outbox missing column {col}");
        }
    }

    #[test]
    fn migrations_are_versioned_in_order() {
        let versions: Vec<&str> = MIGRATIONS.iter().map(|m| m.version).collect();
        assert_eq!(versions, vec!["V001", "V002", "V003", "V004"]);
        let names: Vec<&str> = MIGRATIONS.iter().map(|m| m.name).collect();
        assert_eq!(
            names,
            vec![
                "init",
                "ntf_tenant",
                "ntf_membership_projection",
                "align_bigint_signed"
            ]
        );
    }

    #[test]
    fn marker_tables_match_mysql_side() {
        let markers: Vec<&str> = MIGRATIONS.iter().map(|m| m.marker_table).collect();
        assert_eq!(
            markers,
            vec![
                "ntf_announcements",
                "ntf_migration_v002_applied",
                "ntf_migration_v003_applied",
                "ntf_migration_v004_applied"
            ]
        );
    }

    /// 与 `apply_script` 同口径：剥离 `--` 注释行后再检查——注释永远不会到达
    /// 执行器，文件头部的转换规则说明不构成残留。
    fn executable_sql(script: &str) -> String {
        script
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
            .to_uppercase()
    }

    /// PG 脚本必须用 TIMESTAMPTZ / JSONB，且不得残留 MySQL 专有语法。
    #[test]
    fn postgres_scripts_use_pg_types_only() {
        for m in MIGRATIONS {
            let sql = executable_sql(m.sql_for(DialectKind::Postgres));
            for forbidden in [
                "ON UPDATE CURRENT_TIMESTAMP",
                "ENGINE=",
                "AUTO_INCREMENT",
                "TINYINT",
                "DATETIME",
                "INSERT IGNORE",
                "ON DUPLICATE KEY",
                "UNSIGNED",
            ] {
                assert!(
                    !sql.contains(forbidden),
                    "{} PG script still contains `{forbidden}`",
                    m.version
                );
            }
        }
        // 时间列（announcements/reads/outbox）必须 TIMESTAMPTZ；
        // envelope_json / role_ids 必须是 JSONB（sqlx 以 JSONB 绑定 serde_json）。
        let v001 = executable_sql(MIGRATIONS[0].sql_for(DialectKind::Postgres));
        assert!(v001.contains("TIMESTAMPTZ"), "V001 PG 必须用 TIMESTAMPTZ");
        assert!(
            v001.contains("ENVELOPE_JSON   JSONB"),
            "V001 PG 的 envelope_json 必须是 JSONB"
        );
        let v003 = executable_sql(MIGRATIONS[2].sql_for(DialectKind::Postgres));
        assert!(
            v003.contains("ROLE_IDS       JSONB"),
            "V003 PG 的 role_ids 必须是 JSONB"
        );
    }

    /// 迁移文件的注释里不能有 ASCII 分号，否则切分器会把注释切成可执行 SQL。
    #[test]
    fn migration_comments_have_no_ascii_semicolons() {
        for m in MIGRATIONS {
            for dialect in [DialectKind::Mysql, DialectKind::Postgres] {
                for line in m.sql_for(dialect).lines() {
                    if line.trim_start().starts_with("--") {
                        assert!(
                            !line.contains(';'),
                            "{} {:?} comment has an ASCII semicolon: {line}",
                            m.version,
                            dialect
                        );
                    }
                }
            }
        }
    }
}
