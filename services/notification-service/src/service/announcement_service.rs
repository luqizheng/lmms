use crate::consumer::placeholder::render_template;
use crate::redis_cli::RedisCli;
use crate::ws_hub::{WsHub, WsOut};
use chrono::{DateTime, Utc};
use common_event::events::IamUserCreated;
use sqlx::mysql::MySqlPool;
use sqlx::Row;
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

pub async fn load_welcome_template(pool: &MySqlPool) -> WelcomeTemplate {
    match sqlx::query("SELECT title, body FROM ntf_templates WHERE code = 'welcome'")
        .fetch_optional(pool)
        .await
    {
        Ok(Some(row)) => {
            let title: String = row.try_get("title").unwrap_or_default();
            let body: String = row.try_get("body").unwrap_or_default();
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

pub async fn ensure_welcome_for_user(
    pool: &MySqlPool,
    redis: &RedisCli,
    hub: &WsHub,
    event_id: Uuid,
    user_id: &str,
    payload: &IamUserCreated,
    occurred_at: DateTime<Utc>,
) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;

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
        return Ok(false);
    }

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

    let tpl = load_welcome_template(pool).await;
    let title = render_template(&tpl.title, &vars);
    let body = render_template(&tpl.body, &vars);

    let res = sqlx::query(
        "INSERT INTO ntf_announcements \
         (title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at) \
         VALUES (?, ?, NOW(), NULL, 0, 0, 'published', 'system', NOW())",
    )
    .bind(&title)
    .bind(&body)
    .execute(&mut *tx)
    .await?;
    let ann_id = res.last_insert_id();

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

    tx.commit().await?;

    if let Err(e) = redis.incr_unread(user_id.to_string(), 1).await {
        warn!(error = %e, user_id = %user_id, "incr_unread failed (non-fatal)");
    }
    hub.publish(
        user_id.to_string(),
        WsOut::New(serde_json::json!({
            "id": ann_id,
            "title": title,
            "pinned": false,
            "popover": false,
        })),
    )
    .await;

    info!(event_id = %event_id, user_id = %user_id, ann_id, "welcome announcement created");
    Ok(true)
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
        let payload = common_event::events::IamUserCreated {
            user_id: "u1".to_string(),
            username: String::new(),
            email: None,
            org_unit_id: None,
        };
        let mut vars: std::collections::HashMap<&'static str, String> = std::collections::HashMap::new();
        vars.insert(
            "username",
            if payload.username.is_empty() { "New User".into() } else { payload.username.clone() },
        );
        vars.insert(
            "dept_name",
            match payload.org_unit_id.as_deref() {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => "Platform".into(),
            },
        );
        assert_eq!(vars.get("username").unwrap(), "New User");
        assert_eq!(vars.get("dept_name").unwrap(), "Platform");
    }
}
