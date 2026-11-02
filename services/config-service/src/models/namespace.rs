use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Namespace {
    pub id: String,
    // 全链路 i64：sqlx-postgres 没有任何 unsigned 支持，u64 在 PG 上既不能 bind 也不能解码
    // （MySQL 侧同样已由 V003 迁移改成有符号 BIGINT）。
    //
    // `#[schema(minimum = 0)]` 是为了让 OpenAPI spec 与改造前的 `u64` **零 diff**：
    // utoipa 对 `u64` 自动产出 `minimum: 0`，换成 `i64` 后该约束会消失，需显式声明。
    // ⚠️ 这段说明刻意写成 `//` 而非 `///`——utoipa 会把文档注释渲染进 spec 的
    // `description`，那会给 `openapi.yaml` 引入非预期 diff（CI 有 `git diff --exit-code`）。
    #[schema(minimum = 0)]
    pub tenant_id: i64,
    pub app: String,
    pub env: String,
    pub profile: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateNamespace {
    pub app: String,
    pub env: String,
    pub profile: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNamespace {
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct QueryNamespaces {
    pub app: Option<String>,
    pub env: Option<String>,
    #[serde(default = "default_page")]
    pub page: Option<u32>,
    #[serde(default = "default_size")]
    pub size: Option<u32>,
}

fn default_page() -> Option<u32> {
    Some(1)
}

fn default_size() -> Option<u32> {
    Some(20)
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PagedNamespaces {
    pub items: Vec<Namespace>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
