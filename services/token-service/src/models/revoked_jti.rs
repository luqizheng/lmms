#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RevokedJtiRow {
    pub jti: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
