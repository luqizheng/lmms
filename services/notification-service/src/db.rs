use crate::error::AppResult;
use crate::models::*;
use chrono::{DateTime, Utc};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::collections::HashSet;

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
        let stmts: &[&str] = &[
            "CREATE TABLE IF NOT EXISTS announcements (
                id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
                title VARCHAR(255) NOT NULL,
                body TEXT NOT NULL,
                publish_at DATETIME NOT NULL,
                expire_at DATETIME NULL,
                pinned TINYINT NOT NULL DEFAULT 0,
                popover TINYINT NOT NULL DEFAULT 0,
                status VARCHAR(16) NOT NULL,
                created_by BIGINT UNSIGNED NOT NULL,
                created_at DATETIME NOT NULL,
                PRIMARY KEY (id),
                KEY idx_status_publish (status, publish_at)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS announcement_recipients (
                announcement_id BIGINT UNSIGNED NOT NULL,
                scope_type VARCHAR(16) NOT NULL,
                scope_id BIGINT UNSIGNED NOT NULL,
                PRIMARY KEY (announcement_id, scope_type, scope_id),
                KEY idx_scope (scope_type, scope_id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS announcement_deliveries (
                announcement_id BIGINT UNSIGNED NOT NULL,
                user_id BIGINT UNSIGNED NOT NULL,
                delivered_at DATETIME NOT NULL,
                PRIMARY KEY (announcement_id, user_id),
                KEY idx_user (user_id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS announcement_reads (
                announcement_id BIGINT UNSIGNED NOT NULL,
                user_id BIGINT UNSIGNED NOT NULL,
                read_at DATETIME NOT NULL,
                PRIMARY KEY (announcement_id, user_id),
                KEY idx_user (user_id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        ];
        for s in stmts {
            sqlx::query(s).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn create_announcement(
        &self,
        a: &CreateAnnouncement,
        created_by: u64,
    ) -> AppResult<u64> {
        let mut tx = self.pool.begin().await?;
        let status = if a.publish_at <= Utc::now() {
            Status::Published
        } else {
            Status::Scheduled
        };
        let res = sqlx::query(
            "INSERT INTO announcements \
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
                "INSERT INTO announcement_recipients (announcement_id, scope_type, scope_id) VALUES (?,?,?)"
            )
            .bind(id).bind(s.scope_type).bind(s.scope_id)
            .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn set_status(&self, id: u64, status: Status) -> AppResult<()> {
        sqlx::query("UPDATE announcements SET status=? WHERE id=?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get(&self, id: u64) -> AppResult<Option<Announcement>> {
        Ok(sqlx::query_as::<_, Announcement>(
            "SELECT id, title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at \
             FROM announcements WHERE id=?"
        ).bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn list_recipient_scopes(&self, ann_id: u64) -> AppResult<Vec<(ScopeType, u64)>> {
        let rows: Vec<(String, u64)> = sqlx::query_as(
            "SELECT scope_type, scope_id FROM announcement_recipients WHERE announcement_id=?",
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
            "SELECT id FROM announcements WHERE status='scheduled' AND publish_at <= ?",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn list_for_user(&self, user_id: u64) -> AppResult<Vec<Announcement>> {
        Ok(sqlx::query_as::<_, Announcement>(
            "SELECT a.id, a.title, a.body, a.publish_at, a.expire_at, a.pinned, a.popover, \
                    a.status, a.created_by, a.created_at \
             FROM announcements a \
             JOIN announcement_deliveries d ON d.announcement_id = a.id \
             WHERE d.user_id=? AND a.status='published' \
               AND (a.expire_at IS NULL OR a.expire_at > NOW()) \
             ORDER BY a.pinned DESC, a.publish_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn insert_deliveries(&self, ann_id: u64, users: &HashSet<u64>) -> AppResult<()> {
        if users.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for uid in users {
            sqlx::query(
                "INSERT IGNORE INTO announcement_deliveries (announcement_id, user_id, delivered_at) VALUES (?,?,NOW())"
            ).bind(ann_id).bind(*uid).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_read(&self, ann_id: u64, user_id: u64) -> AppResult<bool> {
        let res = sqlx::query(
            "INSERT IGNORE INTO announcement_reads (announcement_id, user_id, read_at) VALUES (?,?,NOW())"
        ).bind(ann_id).bind(user_id).execute(&self.pool).await?;
        Ok(res.rows_affected() == 1)
    }
}
