use crate::error::{AppError, AppResult};
use crate::models::*;
use chrono::{DateTime, Utc};
use common_event::events::NotificationAnnouncementPublished;
use common_event::{Envelope, MySqlOutboxRepo, OutboxRepo};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::Row;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{info, warn};

const INIT_SQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const SCHEMA_MARKER_TABLE: &str = "ntf_announcements";

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
        if exists > 0 {
            info!(
                marker = SCHEMA_MARKER_TABLE,
                "schema already present, skip migration"
            );
            return Ok(());
        }

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
        for uid in users {
            sqlx::query(
                "INSERT IGNORE INTO ntf_announcement_deliveries (announcement_id, user_id, delivered_at) VALUES (?,?,NOW())"
            ).bind(ann_id).bind(uid).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_read(&self, ann_id: u64, user_id: String) -> AppResult<bool> {
        let res = sqlx::query(
            "INSERT IGNORE INTO ntf_announcement_reads (announcement_id, user_id, read_at) VALUES (?,?,NOW())"
        ).bind(ann_id).bind(user_id).execute(&self.pool).await?;
        Ok(res.rows_affected() == 1)
    }

    /// 公告发布的统一事务管线（REST publish + scheduler 共用）。
    ///
    /// ```text
    /// publish_announcement(ann_id)  ── ONE TX ──
    ///   1. SELECT ... FOR UPDATE 锁行；status ∈ {published, revoked} → 幂等返回 Ok(vec![])（draft/scheduled 继续）
    ///   2. SELECT scope_type='user' recipients → user_ids（role/dept 仅 warn 跳过）
    ///   3. INSERT IGNORE deliveries（每 user_id）
    ///   4. UPDATE status='published'
    ///   5. enqueue NotificationAnnouncementPublished → ntf_outbox_events
    ///   COMMIT
    /// ```
    /// 不在此处 hub.publish —— 推送统一由 announcement consumer 完成（避免双推）。
    pub async fn publish_announcement(&self, ann_id: u64) -> AppResult<Vec<String>> {
        let mut tx = self.pool.begin().await?;

        // 1. 锁行 + 状态守卫
        let row: Option<(String, String, i8, i8)> = sqlx::query_as(
            "SELECT title, status, pinned, popover FROM ntf_announcements WHERE id=? FOR UPDATE",
        )
        .bind(ann_id)
        .fetch_optional(&mut *tx)
        .await?;
        let (title, status, pinned, popover) = match row {
            Some(r) => r,
            None => {
                tx.commit().await?;
                return Ok(vec![]);
            }
        };
        if status == "published" || status == "revoked" {
            tx.commit().await?;
            return Ok(vec![]);
        }

        // 2. user-scope 接收人（role/dept 跳过 — follow-up 4.3-fu2）
        let scope_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT scope_type, scope_id FROM ntf_announcement_recipients WHERE announcement_id=?",
        )
        .bind(ann_id)
        .fetch_all(&mut *tx)
        .await?;
        let (user_ids, skipped_non_user) = filter_user_scope(&scope_rows);
        if skipped_non_user > 0 {
            warn!(
                ann_id,
                skipped_non_user,
                "role/dept recipient scopes skipped (user-scope only; see 4.3-fu2)"
            );
        }

        // 3. deliveries
        for uid in &user_ids {
            sqlx::query(
                "INSERT IGNORE INTO ntf_announcement_deliveries (announcement_id, user_id, delivered_at) VALUES (?,?,NOW())",
            )
            .bind(ann_id)
            .bind(uid)
            .execute(&mut *tx)
            .await?;
        }

        // 4. status
        sqlx::query("UPDATE ntf_announcements SET status='published' WHERE id=?")
            .bind(ann_id)
            .execute(&mut *tx)
            .await?;

        // 5. enqueue
        let envelope = Envelope::new(
            "notification-service",
            NotificationAnnouncementPublished {
                announcement_id: ann_id as i64,
                title,
                pinned: pinned != 0,
                popover: popover != 0,
                user_ids: serde_json::json!(user_ids),
            },
        );
        self.outbox_repo.clone()
            .enqueue(&mut tx, &envelope)
            .await
            .map_err(|e| AppError::from(anyhow::anyhow!("outbox enqueue: {e}")))?;

        tx.commit().await?;
        Ok(user_ids)
    }
}

/// 从接收人 scope 行中筛出 user-scope 目标，返回 (user_ids, 被跳过的 role/dept 数)。
pub(crate) fn filter_user_scope(rows: &[(String, String)]) -> (Vec<String>, usize) {
    let mut users = Vec::new();
    let mut skipped = 0;
    for (st, sid) in rows {
        if st == "user" {
            users.push(sid.clone());
        } else {
            skipped += 1;
        }
    }
    (users, skipped)
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty() && !l.trim().starts_with("--"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
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
    fn filter_user_scope_keeps_users_and_counts_others() {
        let rows = vec![
            ("user".to_string(), "u1".to_string()),
            ("role".to_string(), "r1".to_string()),
            ("user".to_string(), "u2".to_string()),
            ("dept".to_string(), "d1".to_string()),
        ];
        let (users, skipped) = filter_user_scope(&rows);
        assert_eq!(users, vec!["u1", "u2"]);
        assert_eq!(skipped, 2);
    }

    #[test]
    fn init_sql_outbox_columns() {
        let upper = INIT_SQL.to_uppercase();
        for col in ["EVENT_ID", "EVENT_TYPE", "ROUTING_KEY", "ENVELOPE_JSON", "PUBLISHED_AT"] {
            assert!(upper.contains(col), "outbox missing column {col}");
        }
    }
}
