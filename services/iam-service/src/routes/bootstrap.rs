//! Bootstrap HTTP API（WP-01E）。
//!
//! `GET  /v1/bootstrap/status` — 查询初始化状态（密码校验同 POST）。
//! `POST /v1/bootstrap`        — 提交初始化 / 触发恢复。
//!
//! 状态码契约（设计 Q4，不得另行定义）：
//!
//! | 情况 | 状态码 |
//! |---|---:|
//! | 未配置 `LMMS_INIT_PASSWORD` / Bootstrap 已完成 | 404 |
//! | 初始化密码错误 | 403 |
//! | 请求参数不合法 | 400 |
//! | 窗口进行中 / 非胜出 / 租约活跃 / 步骤冲突 | 409 |
//! | 步骤失败（依赖不可用） | 502 |
//! | 步骤失败（其他） | 500 |
//! | 全部步骤完成 | 200 |
//!
//! 安全：`X-LMMS-Init-Password` 只在本模块使用，**永不**写日志、不进
//! 响应、不持久化；错误响应体不含密码或内部拓扑细节。本组路由不挂
//! JWT 中间件（部署初期无身份可言，凭密码准入）。

use crate::dto::{BootstrapRequest, BootstrapResponse, BootstrapStatusResponse};
use crate::service::bootstrap::{BootstrapService, INIT_PASSWORD_HEADER};
use crate::state::AppState;
use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use digua_common_error::AppError;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/v1/bootstrap/status", get(get_status))
        .route("/v1/bootstrap", post(post_bootstrap))
        .with_state(state)
}

/// 提取专用 Header（值不进任何日志）。
fn extract_password(headers: &HeaderMap) -> Option<String> {
    headers
        .get(INIT_PASSWORD_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[utoipa::path(
    get,
    path = "/v1/bootstrap/status",
    responses(
        (status = 200, body = BootstrapStatusResponse),
        (status = 403, description = "invalid bootstrap password"),
        (status = 404, description = "bootstrap disabled or already completed"),
    ),
    tag = "bootstrap"
)]
pub async fn get_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<BootstrapStatusResponse>, AppError> {
    let password = extract_password(&headers);
    BootstrapService::new(state)
        .status(password)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/v1/bootstrap",
    request_body = BootstrapRequest,
    responses(
        (status = 200, body = BootstrapResponse),
        (status = 400, description = "invalid bootstrap request"),
        (status = 403, description = "invalid bootstrap password"),
        (status = 404, description = "bootstrap disabled or already completed"),
        (status = 409, description = "admission conflict / lease active / step conflict"),
        (status = 500, description = "bootstrap step failed"),
        (status = 502, description = "bootstrap dependency unavailable"),
    ),
    tag = "bootstrap"
)]
pub async fn post_bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BootstrapRequest>,
) -> Result<Json<BootstrapResponse>, AppError> {
    let password = extract_password(&headers);
    // 密码只经 extract_password → service；任何失败路径都不打 Header 日志。
    BootstrapService::new(state)
        .bootstrap(req, password)
        .await
        .map(Json)
        .map_err(Into::into)
}
