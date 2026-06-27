use sqlx::mysql::MySqlPool;
use sqlx::Row;
use tracing::warn;

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
}
