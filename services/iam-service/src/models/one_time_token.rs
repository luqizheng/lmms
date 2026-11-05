//! One-time tokens for password reset and invitation acceptance.
//!
//! Only the SHA-256 hash is persisted; the plaintext token appears once in the
//! outbound mail link.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct OneTimeToken {
    pub id: String,
    pub tenant_id: i64,
    pub user_id: String,
    pub kind: String,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}