use crate::error::AppResult;
use crate::models::*;
use chrono::{DateTime, Utc};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::Row;
use std::collections::HashSet;
use tracing::{info, warn};

const INIT_SQL: &str = include_str!("../migrations/2026_06_10_init.sql");
const USER_ID_STRING_SQL: &str = include_str!("../migrations/2026_06_21_user_id_string.sql");
const WELCOME_TEMPLATE_SQL: &str = include_str!("../migrations/2026_06_27_welcome_template.sql");
const SCHEMA_MARKER_TABLE: &str = "ntf_announcements";

#[derive(Clone)]
pub struct Db {
    pub pool: MySqlPool,
}

impl Db {
    pub async fn connect(url: &str, max: u32) -> anyhow::Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(max)
            .connect(url)
            .await?;
        Ok(Self { pool })
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
            // sqlx 连接已由 cfg.mysql.url 指定数据库；
            // 跳过文件中的 CREATE DATABASE / USE（连接池下不应切库）。
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
        
        // Apply user_id string migration
        for stmt in USER_ID_STRING_SQL.split(';') {
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
                warn!(statement = %first_line(s), error = %e, "user_id string migration statement failed (may already be applied)");
            }
        }
        info!("user_id string migration applied");

        for stmt in WELCOME_TEMPLATE_SQL.split(';') {
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
                warn!(statement = %first_line(s), error = %e, "welcome template migration statement failed (may already be applied)");
            }
        }
        info!("welcome template migration applied");

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
    fn welcome_template_sql_contains_required_tables_and_seed() {
        let upper = WELCOME_TEMPLATE_SQL.to_uppercase();
        assert!(upper.contains("CREATE TABLE IF NOT EXISTS NTF_TEMPLATES"));
        assert!(upper.contains("CREATE TABLE IF NOT EXISTS NTF_EVENT_IDEMPOTENCY"));
        assert!(upper.contains("INSERT INTO NTF_TEMPLATES"));
        assert!(WELCOME_TEMPLATE_SQL.contains("'welcome'"));
    }
}
