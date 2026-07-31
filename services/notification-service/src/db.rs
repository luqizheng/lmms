use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::{DateTime, Utc};
use digua_common_db::WithTenantScope;
use digua_common_event::events::NotificationAnnouncementPublished;
use digua_common_event::{Envelope, MySqlOutboxRepo, OutboxRepo};
use digua_jwt_ctx::TenantScope;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::{QueryBuilder, Row};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{info, warn};

const INIT_SQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const V002_SQL: &str = include_str!("../migrations/mysql/V002__ntf_tenant.sql");
const V003_SQL: &str = include_str!("../migrations/mysql/V003__ntf_membership_projection.sql");
const SCHEMA_MARKER_TABLE: &str = "ntf_announcements";
const V002_MARKER_TABLE: &str = "ntf_migration_v002_applied";
const V003_MARKER_TABLE: &str = "ntf_migration_v003_applied";

#[derive(Clone)]
pub struct Db {
    pub pool: MySqlPool,
    pub outbox_repo: Arc<MySqlOutboxRepo>,
}

impl Db {
    pub async fn connect(url: &str, max: u32) -> anyhow::Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(max)
            .connect(url)
            .await?;
        let outbox_repo = Arc::new(MySqlOutboxRepo::with_table(
            pool.clone(),
            "ntf_outbox_events",
        ));
        Ok(Self { pool, outbox_repo })
    }

    pub async fn ensure_schema(&self) -> anyhow::Result<()> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS c FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = ?",
        )
        .bind(SCHEMA_MARKER_TABLE)
        .fetch_one(&self.pool)
        .await?;
        let exists: i64 = row.try_get("c").unwrap_or(0);
        if exists == 0 {
            info!(
                marker = SCHEMA_MARKER_TABLE,
                "schema missing, applying initial migration"
            );
            for stmt in INIT_SQL.split(';') {
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
                if upper.starts_with("CREATE DATABASE") || upper.starts_with("USE ") {
                    continue;
                }
                if let Err(e) = sqlx::query(s).execute(&self.pool).await {
                    warn!(statement = %first_line(s), error = %e, "migration statement failed");
                    return Err(anyhow::anyhow!("apply migration ({}): {e}", first_line(s)));
                }
            }
            info!("initial migration applied");
        } else {
            info!(
                marker = SCHEMA_MARKER_TABLE,
                "schema already present, skip V001"
            );
        }

        if !table_exists(&self.pool, V002_MARKER_TABLE).await? {
            info!(marker = V002_MARKER_TABLE, "applying V002");
            for stmt in V002_SQL.split(';') {
                let cleaned: String = stmt
                    .lines()
                    .filter(|l| !l.trim_start().starts_with("--"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let s = cleaned.trim();
                if s.is_empty() {
                    continue;
                }
                if let Err(e) = sqlx::query(s).execute(&self.pool).await {
                    warn!(statement = %first_line(s), error = %e, "V002 statement failed");
                    return Err(anyhow::anyhow!("apply V002 ({}): {e}", first_line(s)));
                }
            }
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS ntf_migration_v002_applied \
                 (applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            )
            .execute(&self.pool)
            .await?;
            info!("V002 applied");
        }

        if !table_exists(&self.pool, V003_MARKER_TABLE).await? {
            info!(marker = V003_MARKER_TABLE, "applying V003");
            for stmt in V003_SQL.split(';') {
                let cleaned: String = stmt
                    .lines()
                    .filter(|l| !l.trim_start().starts_with("--"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let s = cleaned.trim();
                if s.is_empty() {
                    continue;
                }
                if let Err(e) = sqlx::query(s).execute(&self.pool).await {
                    warn!(statement = %first_line(s), error = %e, "V003 statement failed");
                    return Err(anyhow::anyhow!("apply V003 ({}): {e}", first_line(s)));
                }
            }
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS ntf_migration_v003_applied \
                 (applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            )
            .execute(&self.pool)
            .await?;
            info!("V003 applied");
        }

        Ok(())
    }

    pub async fn create_announcement(
        &self,
        a: &CreateAnnouncement,
        created_by: String,
    ) -> AppResult<u64> {
        let mut tx = self.pool.begin().await?;
        let status = if a.publish_at <= Utc::now() {
            Status::Published
        } else {
            Status::Scheduled
        };
        let res = sqlx::query(
            "INSERT INTO ntf_announcements \
             (title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at) \
             VALUES (?,?,?,?,?,?,?,?,NOW())",
        )
        .bind(&a.title)
        .bind(&a.body)
        .bind(a.publish_at)
        .bind(a.expire_at)
        .bind(a.pinned as i8)
        .bind(a.popover as i8)
        .bind(status)
        .bind(created_by)
        .execute(&mut *tx)
        .await?;
        let id = res.last_insert_id();
        for s in &a.recipients {
            sqlx::query(
                "INSERT INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) VALUES (?,?,?)"
            )
            .bind(id).bind(s.scope_type).bind(&s.scope_id)
            .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn set_status(&self, id: u64, status: Status) -> AppResult<()> {
        sqlx::query("UPDATE ntf_announcements SET status=? WHERE id=?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get(&self, id: u64) -> AppResult<Option<Announcement>> {
        Ok(sqlx::query_as::<_, Announcement>(
            "SELECT id, title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at \
             FROM ntf_announcements WHERE id=?"
        ).bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn list_recipient_scopes(&self, ann_id: u64) -> AppResult<Vec<(ScopeType, String)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT scope_type, scope_id FROM ntf_announcement_recipients WHERE announcement_id=?",
        )
        .bind(ann_id)
        .fetch_all(&self.pool)
        .await?;
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

    pub async fn due_scheduled(&self, now: DateTime<Utc>) -> AppResult<Vec<u64>> {
        let rows: Vec<(u64,)> = sqlx::query_as(
            "SELECT id FROM ntf_announcements WHERE status='scheduled' AND publish_at <= ?",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn list_for_user(&self, user_id: String) -> AppResult<Vec<Announcement>> {
        Ok(sqlx::query_as::<_, Announcement>(
            "SELECT a.id, a.title, a.body, a.publish_at, a.expire_at, a.pinned, a.popover, \
                    a.status, a.created_by, a.created_at \
             FROM ntf_announcements a \
             JOIN ntf_announcement_deliveries d ON d.announcement_id = a.id \
             WHERE d.user_id=? AND a.status='published' \
               AND (a.expire_at IS NULL OR a.expire_at > NOW()) \
             ORDER BY a.pinned DESC, a.publish_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn insert_deliveries(&self, ann_id: u64, users: &HashSet<String>) -> AppResult<()> {
        if users.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        let v: Vec<String> = users.iter().cloned().collect();
        insert_deliveries_batch(ann_id, &v, &mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_read(&self, ann_id: u64, user_id: String) -> AppResult<bool> {
        let res = sqlx::query(
            "INSERT IGNORE INTO ntf_announcement_reads (announcement_id, user_id, read_at) VALUES (?,?,NOW())"
        ).bind(ann_id).bind(user_id).execute(&self.pool).await?;
        Ok(res.rows_affected() == 1)
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
        ann_id: u64,
        scope: &TenantScope,
    ) -> AppResult<Vec<String>> {
        let (title, pinned, popover) = {
            let mut tx = self.pool.begin().await?;
            let mut qb = QueryBuilder::new(
                "SELECT title, status, pinned, popover FROM ntf_announcements",
            );
            qb.where_id(ann_id);
            qb.and_tenant(scope);
            qb.push(" FOR UPDATE");
            let row: Option<(String, String, i8, i8)> =
                qb.build_query_as::<(String, String, i8, i8)>()
                    .fetch_optional(&mut *tx)
                    .await?;
            match row {
                Some((title, status, pinned, popover)) => {
                    if status == "published" || status == "revoked" {
                        tx.commit().await?;
                        return Ok(vec![]);
                    }
                    tx.commit().await?;
                    (title, pinned, popover)
                }
                None => {
                    tx.commit().await?;
                    return Ok(vec![]);
                }
            }
        };

        let scopes = self.list_recipient_scopes(ann_id).await?;
        let user_ids: HashSet<String> = if scopes.is_empty() {
            HashSet::new()
        } else {
            self.resolve_users_by_scopes(&scopes, scope).await?
        };

        let user_ids_vec: Vec<String> = user_ids.iter().cloned().collect();
        let mut tx = self.pool.begin().await?;
        let mut qb = QueryBuilder::new("SELECT status FROM ntf_announcements");
        qb.where_id(ann_id);
        qb.and_tenant(scope);
        qb.push(" FOR UPDATE");
        let (status_again,): (String,) = qb
            .build_query_as::<(String,)>()
            .fetch_one(&mut *tx)
            .await?;
        if status_again == "published" || status_again == "revoked" {
            return Ok(user_ids_vec);
        }
        insert_deliveries_batch(ann_id, &user_ids_vec, &mut tx).await?;
        let mut qb = QueryBuilder::new("UPDATE ntf_announcements SET status='published'");
        qb.where_id(ann_id);
        qb.and_tenant(scope);
        qb.build()
            .execute(&mut *tx)
            .await?;
        let mut envelope = Envelope::new(
            "notification-service",
            NotificationAnnouncementPublished {
                announcement_id: ann_id as i64,
                title,
                pinned: pinned != 0,
                popover: popover != 0,
                user_ids: serde_json::json!(user_ids),
            },
        );
        self.outbox_repo
            .clone()
            .enqueue(&mut tx, &mut envelope)
            .await
            .map_err(|e| AppError::from(anyhow::anyhow!("outbox enqueue: {e}")))?;
        tx.commit().await?;
        Ok(user_ids_vec)
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
        let mut tx = self.pool.begin().await?;
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
        let mut tx = self.pool.begin().await?;
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
        let mut tx = self.pool.begin().await?;
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
        let mut tx = self.pool.begin().await?;
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
                    let rows: Vec<(String,)> = sqlx::query_as(
                        "SELECT user_id FROM ntf_user_projection \
                         WHERE tenant_id = ? \
                           AND JSON_CONTAINS(role_ids, JSON_QUOTE(?))",
                    )
                    .bind(tenant_id)
                    .bind(id)
                    .fetch_all(&self.pool)
                    .await?;
                    out.extend(rows.into_iter().map(|(s,)| s));
                }
                crate::models::ScopeType::Dept => {
                    let rows: Vec<(String,)> = sqlx::query_as(
                        "SELECT user_id FROM ntf_user_projection \
                         WHERE tenant_id = ? AND dept_id = ?",
                    )
                    .bind(tenant_id)
                    .bind(id)
                    .fetch_all(&self.pool)
                    .await?;
                    out.extend(rows.into_iter().map(|(s,)| s));
                }
            }
        }
        Ok(out)
    }
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty() && !l.trim().starts_with("--"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
}

async fn table_exists(pool: &MySqlPool, table: &str) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

async fn insert_deliveries_batch(
    ann_id: u64,
    users: &[String],
    conn: &mut sqlx::MySqlConnection,
) -> AppResult<()> {
    if users.is_empty() {
        return Ok(());
    }
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT IGNORE INTO ntf_announcement_deliveries \
         (announcement_id, user_id, delivered_at) ",
    );
    qb.push_values(users.iter(), |mut b, uid| {
        b.push_bind(ann_id as i64).push_bind(uid).push("NOW()");
    });
    qb.build().execute(&mut *conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_sql_contains_all_tables() {
        let upper = INIT_SQL.to_uppercase();
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
                "missing CREATE TABLE for {tbl}"
            );
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
        let upper = INIT_SQL.to_uppercase();
        assert!(upper.contains("INSERT INTO NTF_TEMPLATES"));
        assert!(INIT_SQL.contains("'welcome'"));
    }

    #[test]
    fn init_sql_outbox_columns() {
        let upper = INIT_SQL.to_uppercase();
        for col in ["EVENT_ID", "EVENT_TYPE", "ROUTING_KEY", "ENVELOPE_JSON", "PUBLISHED_AT"] {
            assert!(upper.contains(col), "outbox missing column {col}");
        }
    }
}
