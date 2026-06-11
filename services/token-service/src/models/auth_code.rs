#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthCodeRow {
    pub code: String,
    pub user_id: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub consumed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
