use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Draft,
    Scheduled,
    Published,
    Revoked,
}

impl Status {
    fn as_db_str(&self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Scheduled => "scheduled",
            Status::Published => "published",
            Status::Revoked => "revoked",
        }
    }

    fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Status::Draft),
            "scheduled" => Some(Status::Scheduled),
            "published" => Some(Status::Published),
            "revoked" => Some(Status::Revoked),
            _ => None,
        }
    }
}

// 手写 sqlx 映射：strong-enum derive 的 `compatible` 对 MySQL `ENUM` 列做严格
// flag 相等比较（NOT NULL ENUM 列带额外 flag），导致 `Announcement.status` 解码
// 始终失败。委派给 `str` 的宽松兼容（其接受 ColumnType::Enum）修正之。
impl sqlx::Type<sqlx::MySql> for Status {
    fn type_info() -> sqlx::mysql::MySqlTypeInfo {
        <str as sqlx::Type<sqlx::MySql>>::type_info()
    }
    fn compatible(ty: &sqlx::mysql::MySqlTypeInfo) -> bool {
        <str as sqlx::Type<sqlx::MySql>>::compatible(ty)
    }
}

impl sqlx::Encode<'_, sqlx::MySql> for Status {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::MySql as sqlx::Database>::ArgumentBuffer<'_>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&str as sqlx::Encode<sqlx::MySql>>::encode(self.as_db_str(), buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::MySql> for Status {
    fn decode(
        value: sqlx::mysql::MySqlValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::MySql>>::decode(value)?;
        Status::from_db_str(s).ok_or_else(|| format!("invalid announcement status: {s}").into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ScopeType {
    User,
    Role,
    Dept,
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
pub struct Announcement {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    pub pinned: i8,
    pub popover: i8,
    pub status: Status,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ScopeInput {
    pub scope_type: ScopeType,
    pub scope_id: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateAnnouncement {
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    pub pinned: bool,
    pub popover: bool,
    pub recipients: Vec<ScopeInput>,
}

/// Phase 5B-fu3: bootstrap snapshot payload row (per-user membership).
/// Mirrors `iam-service::routes::internal_membership::UserMembership`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct UserMembership {
    pub role_ids: Vec<String>,
    pub dept_id: Option<String>,
}
