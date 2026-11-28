//! Bootstrap 初始化 API DTO（WP-BOOTSTRAP-01 / WP-01E）。
//!
//! 契约要点（设计基线：2026-11-28-bootstrap-concurrency-control-design.md）：
//! - 请求体只携带首管理员信息；**不得**出现内部服务身份 / audience / Grant /
//!   内部授权拓扑字段（LMMS 自主定义，见设计 Q11/Q12）。
//! - Bootstrap Password 走专用 Header `X-LMMS-Init-Password`，不进请求体、
//!   不进日志、不持久化。
//! - 首管理员密码只在 `first_admin_user` Step 未完成时必填（恢复时若该
//!   Step 已完成则不需要重新提交），且从不写入响应或 steps JSON。

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// POST /v1/bootstrap 请求体。
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct BootstrapRequest {
    /// 首管理员用户名（3..=50，字母数字下划线）。`first_admin_user` Step
    /// 未完成时必填。
    #[validate(length(min = 3, max = 50))]
    #[serde(default)]
    pub username: Option<String>,
    /// 首管理员密码（8..=128）。仅在首管理员 Step 未完成时必填；
    /// 服务端只用于 argon2 哈希 / 既有用户核对，绝不持久化明文。
    #[validate(length(min = 8, max = 128))]
    #[serde(default)]
    pub password: Option<String>,
    /// 首管理员邮箱（可选）。
    #[validate(length(max = 255))]
    #[serde(default)]
    pub email: Option<String>,
    /// 首管理员显示名（可选，缺省用用户名）。
    #[validate(length(max = 100))]
    #[serde(default)]
    pub nickname: Option<String>,
}

/// 单个初始化 Step 的对外视图（无敏感字段）。
#[derive(Debug, Serialize, ToSchema)]
pub struct BootstrapStepView {
    pub key: String,
    pub status: String,
    pub attempts: u32,
}

/// GET /v1/bootstrap/status 响应。
#[derive(Debug, Serialize, ToSchema)]
pub struct BootstrapStatusResponse {
    /// `pending`（尚无胜出请求）/ `initializing`（胜出流程进行中）/
    /// `failed`（上次执行失败，等待人工恢复）。
    pub status: String,
    pub steps: Vec<BootstrapStepView>,
}

/// POST /v1/bootstrap 成功响应（全部 Step 完成后才返回 200）。
#[derive(Debug, Serialize, ToSchema)]
pub struct BootstrapResponse {
    pub status: String,
    /// 首管理员用户名（非敏感；恢复场景可能为 None）。
    pub admin_username: Option<String>,
    pub steps: Vec<BootstrapStepView>,
}
