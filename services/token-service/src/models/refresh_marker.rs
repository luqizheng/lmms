#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RefreshMarkerRow {
    pub jti: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
