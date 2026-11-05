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
// PG 侧同款（PG 迁移里 ENUM 落地为 VARCHAR，str 兼容 TEXT/VARCHAR）。
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
    fn decode(value: sqlx::mysql::MySqlValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::MySql>>::decode(value)?;
        Status::from_db_str(s).ok_or_else(|| format!("invalid announcement status: {s}").into())
    }
}

#[cfg(feature = "postgres")]
impl sqlx::Type<sqlx::Postgres> for Status {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <str as sqlx::Type<sqlx::Postgres>>::type_info()
    }
    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

#[cfg(feature = "postgres")]
impl sqlx::Encode<'_, sqlx::Postgres> for Status {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'_>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(self.as_db_str(), buf)
    }
}

#[cfg(feature = "postgres")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Status {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Status::from_db_str(s).ok_or_else(|| format!("invalid announcement status: {s}").into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScopeType {
    User,
    Role,
    Dept,
}

impl ScopeType {
    fn as_db_str(&self) -> &'static str {
        match self {
            ScopeType::User => "user",
            ScopeType::Role => "role",
            ScopeType::Dept => "dept",
        }
    }

    fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(ScopeType::User),
            "role" => Some(ScopeType::Role),
            "dept" => Some(ScopeType::Dept),
            _ => None,
        }
    }
}

// 手写 sqlx 映射（与 Status 同款，委派给 `str` 的宽松兼容）：
// 原 `derive(sqlx::Type)` 的 PG 臂按 `type_name` 严格匹配**自定义**类型名，
// 对普通 VARCHAR 列会 compatible 失败 —— 两侧统一改为委派 str。
impl sqlx::Type<sqlx::MySql> for ScopeType {
    fn type_info() -> sqlx::mysql::MySqlTypeInfo {
        <str as sqlx::Type<sqlx::MySql>>::type_info()
    }
    fn compatible(ty: &sqlx::mysql::MySqlTypeInfo) -> bool {
        <str as sqlx::Type<sqlx::MySql>>::compatible(ty)
    }
}

impl sqlx::Encode<'_, sqlx::MySql> for ScopeType {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::MySql as sqlx::Database>::ArgumentBuffer<'_>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&str as sqlx::Encode<sqlx::MySql>>::encode(self.as_db_str(), buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::MySql> for ScopeType {
    fn decode(value: sqlx::mysql::MySqlValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::MySql>>::decode(value)?;
        ScopeType::from_db_str(s).ok_or_else(|| format!("invalid scope type: {s}").into())
    }
}

#[cfg(feature = "postgres")]
impl sqlx::Type<sqlx::Postgres> for ScopeType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <str as sqlx::Type<sqlx::Postgres>>::type_info()
    }
    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

#[cfg(feature = "postgres")]
impl sqlx::Encode<'_, sqlx::Postgres> for ScopeType {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'_>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(self.as_db_str(), buf)
    }
}

#[cfg(feature = "postgres")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ScopeType {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        ScopeType::from_db_str(s).ok_or_else(|| format!("invalid scope type: {s}").into())
    }
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
pub struct Announcement {
    // 全链路 i64：sqlx-postgres 无任何 unsigned 支持，u64 既不能 bind 也不能解码
    // （S1 清单 #5；MySQL 侧 V001 的 BIGINT UNSIGNED 列保持原状，取值远小于 i64::MAX）。
    //
    // `#[schema(minimum = 0)]` 是为了让 OpenAPI spec 与改造前的 `u64` **零 diff**：
    // utoipa 对 `u64` 自动产出 `minimum: 0`，换成 `i64` 后该约束会消失，需显式声明
    // （config-service Namespace.tenant_id 同款处置）。
    // ⚠️ 这段说明刻意写成 `//` 而非 `///`——utoipa 会把文档注释渲染进 spec 的
    // `description`，那会给 `openapi.yaml` 引入非预期 diff（CI 有 `git diff --exit-code`）。
    #[schema(minimum = 0)]
    pub id: i64,
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    // 布尔列 Rust 侧一律 bool（D-5：MySQL TINYINT(1) 与 PG BOOLEAN 双侧都可解码 bool）。
    // `#[schema(value_type = i8)]` 保持 OpenAPI 与改造前 `i8` 零 diff。
    #[schema(value_type = i8)]
    pub pinned: bool,
    #[schema(value_type = i8)]
    pub popover: bool,
    pub status: Status,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    /// Whether the requesting user has read this announcement.
    /// Filled by the service layer from `ntf_announcement_reads`, not selected
    /// from the announcement row itself.
    #[sqlx(default)]
    pub read: bool,
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
