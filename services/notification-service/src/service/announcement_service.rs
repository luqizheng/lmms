use crate::consumer::placeholder::render_template;
use crate::redis_cli::RedisCli;
use chrono::{DateTime, Utc};
use digua_common_db::db_fetch_optional;
use digua_common_db::DbPool;
use digua_common_event::events::{IamUserCreated, NotificationAnnouncementPublished};
use digua_common_event::{AnyTx, Envelope, MySqlOutboxRepo, OutboxRepo};
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

pub struct WelcomeTemplate {
    pub title: String,
    pub body: String,
}

impl WelcomeTemplate {
    pub fn fallback() -> Self {
        Self {
            title: "Welcome to the platform, {{username}}!".to_string(),
            body: "Hi {{username}}, welcome aboard. You have been added to {{dept_name}} on {{created_at}}. Have a great start!".to_string(),
        }
    }
}

/// 读取 welcome 模板（方言无关：`db_*!` 宏负责占位符改写与 bind 校验）。
/// 任何失败都降级为 fallback（模板缺失不阻塞欢迎公告）。
pub async fn load_welcome_template(pool: &DbPool) -> WelcomeTemplate {
    match db_fetch_optional!(
        pool,
        (String, String),
        "SELECT title, body FROM ntf_templates WHERE code = 'welcome'"
    ) {
        Ok(Some((title, body))) => {
            if title.is_empty() || body.is_empty() {
                warn!("welcome template row empty, using fallback");
                WelcomeTemplate::fallback()
            } else {
                WelcomeTemplate { title, body }
            }
        }
        Ok(None) => {
            warn!("welcome template not found, using fallback");
            WelcomeTemplate::fallback()
        }
        Err(e) => {
            warn!(error = %e, "welcome template query failed, using fallback");
            WelcomeTemplate::fallback()
        }
    }
}

fn welcome_vars(
    payload: &IamUserCreated,
    occurred_at: DateTime<Utc>,
) -> HashMap<&'static str, String> {
    let mut vars: HashMap<&'static str, String> = HashMap::new();
    vars.insert(
        "username",
        if payload.username.is_empty() {
            "New User".to_string()
        } else {
            payload.username.clone()
        },
    );
    vars.insert(
        "dept_name",
        match payload.org_unit_id.as_deref() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => "Platform".to_string(),
        },
    );
    vars.insert(
        "created_at",
        if occurred_at.timestamp() <= 0 {
            "Just now".to_string()
        } else {
            occurred_at.format("%Y-%m-%d %H:%M UTC").to_string()
        },
    );
    vars
}

/// iam.user.created → 欢迎公告（幂等：ntf_event_idempotency 吸收重复投递）。
///
/// 整个写入链（幂等标记 + 公告 + recipient + delivery + outbox enqueue）必须
/// 原子生效，因此是**一个事务**。基座层没有跨方言事务抽象
/// （`sqlx::Transaction<DB>` 的 `DB` 是具体类型），沿用 prue_backend 的
/// 显式双臂样板：每臂各自 `begin()` / `commit()`，臂内用原生 `sqlx::query`。
pub async fn ensure_welcome_for_user(
    pool: &DbPool,
    redis: &RedisCli,
    outbox_repo: &MySqlOutboxRepo,
    event_id: Uuid,
    user_id: &str,
    payload: &IamUserCreated,
    occurred_at: DateTime<Utc>,
) -> anyhow::Result<bool> {
    let vars = welcome_vars(payload, occurred_at);
    let tpl = load_welcome_template(pool).await;
    let title = render_template(&tpl.title, &vars);
    let body = render_template(&tpl.body, &vars);

    // Some(ann_id) = 本次创建了公告；None = 幂等命中（重复投递短路）。
    let created: Option<i64> = match pool {
        #[cfg(feature = "mysql")]
        DbPool::Mysql(mysql) => {
            let mut tx = mysql.begin().await?;

            let ins = sqlx::query(
                "INSERT INTO ntf_event_idempotency (event_id, event_type) \
                 VALUES (?, 'iam.user.created') \
                 ON DUPLICATE KEY UPDATE event_id = event_id",
            )
            .bind(event_id.to_string())
            .execute(&mut *tx)
            .await?;
            if ins.rows_affected() == 0 {
                tx.commit().await?;
                info!(event_id = %event_id, user_id = %user_id, "welcome event already processed, skipping");
                None
            } else {
                let res = sqlx::query(
                "INSERT INTO ntf_announcements \
                 (title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at) \
                 VALUES (?, ?, NOW(), NULL, 0, 0, 'published', 'system', NOW())",
            )
            .bind(&title)
            .bind(&body)
            .execute(&mut *tx)
            .await?;
                let ann_id = res.last_insert_id() as i64;

                sqlx::query(
                "INSERT INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) \
                 VALUES (?, 'user', ?)",
            )
            .bind(ann_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

                sqlx::query(
                "INSERT IGNORE INTO ntf_announcement_deliveries (announcement_id, user_id, delivered_at) \
                 VALUES (?, ?, NOW())",
            )
            .bind(ann_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

                let mut envelope = Envelope::new(
                    "notification-service",
                    NotificationAnnouncementPublished {
                        announcement_id: ann_id,
                        title,
                        pinned: false,
                        popover: false,
                        user_ids: serde_json::json!([user_id]),
                    },
                );
                outbox_repo
                    .enqueue(&mut AnyTx::Mysql(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| anyhow::anyhow!("outbox enqueue: {e}"))?;

                tx.commit().await?;
                Some(ann_id)
            }
        }
        #[cfg(feature = "postgres")]
        DbPool::Postgres(pg) => {
            let mut tx = pg.begin().await?;

            // 归属表 #7/#8：MySQL `ON DUPLICATE KEY UPDATE x = x` 的 PG 对应物是
            // `ON CONFLICT (<目标列>) DO NOTHING`（幂等忽略，不做无意义空更新）。
            let ins = sqlx::query(
                "INSERT INTO ntf_event_idempotency (event_id, event_type) \
                 VALUES ($1, 'iam.user.created') \
                 ON CONFLICT (event_id) DO NOTHING",
            )
            .bind(event_id.to_string())
            .execute(&mut *tx)
            .await?;
            if ins.rows_affected() == 0 {
                tx.commit().await?;
                info!(event_id = %event_id, user_id = %user_id, "welcome event already processed, skipping");
                None
            } else {
                // PG 无 last_insert_id()：IDENTITY 主键经 RETURNING id 取回（归属表 #13）。
                // 布尔列直接写 FALSE 字面量（PG 的 BOOLEAN 不接受 0/1）。
                let (ann_id,): (i64,) = sqlx::query_as(
                "INSERT INTO ntf_announcements \
                 (title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at) \
                 VALUES (?, ?, NOW(), NULL, FALSE, FALSE, 'published', 'system', NOW()) RETURNING id",
            )
            .bind(&title)
            .bind(&body)
            .fetch_one(&mut *tx)
            .await?;

                sqlx::query(
                "INSERT INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) \
                 VALUES ($1, 'user', $2)",
            )
            .bind(ann_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

                sqlx::query(
                "INSERT INTO ntf_announcement_deliveries (announcement_id, user_id, delivered_at) \
                 VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING",
            )
            .bind(ann_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

                let mut envelope = Envelope::new(
                    "notification-service",
                    NotificationAnnouncementPublished {
                        announcement_id: ann_id,
                        title,
                        pinned: false,
                        popover: false,
                        user_ids: serde_json::json!([user_id]),
                    },
                );
                outbox_repo
                    .enqueue(&mut AnyTx::Postgres(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| anyhow::anyhow!("outbox enqueue: {e}"))?;

                tx.commit().await?;
                Some(ann_id)
            }
        }
        other => {
            return Err(anyhow::anyhow!(
                "notification-service: dialect {other:?} is not enabled for ensure_welcome_for_user"
            ))
        }
    };

    match created {
        None => Ok(false),
        Some(ann_id) => {
            if let Err(e) = redis.incr_unread(user_id.to_string(), 1).await {
                warn!(error = %e, user_id = %user_id, "incr_unread failed (non-fatal)");
            }

            info!(event_id = %event_id, user_id = %user_id, ann_id, "welcome announcement created");
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_template_is_non_empty() {
        let t = WelcomeTemplate::fallback();
        assert!(!t.title.is_empty());
        assert!(!t.body.is_empty());
        assert!(t.body.contains("{{username}}"));
        assert!(t.body.contains("{{dept_name}}"));
        assert!(t.body.contains("{{created_at}}"));
    }

    #[test]
    fn placeholder_defaults_kick_in_for_empty_payload() {
        let payload = digua_common_event::events::IamUserCreated {
            user_id: "u1".to_string(),
            username: String::new(),
            email: None,
            org_unit_id: None,
        };
        let vars = welcome_vars(&payload, Utc::now());
        assert_eq!(vars.get("username").unwrap(), "New User");
        assert_eq!(vars.get("dept_name").unwrap(), "Platform");
    }
}
