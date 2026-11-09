//! Phase 3 (web): `token_bootstrap_tokens` 的 **Web UI 用**管理端点。
//!
//! 鉴权：user-jwt（浏览器登录后由 gateway 签发）+ 权限码校验。
//! 与 `/admin/bootstrap-tokens*`（service-jwt，给 CI / 运维脚本）共用 `ops` 业务层。
//!
//! 路由前缀 `/v1/admin/*` —— 与 api-gateway 的 `token.router.toml` 约定一致，
//! 网关把 `/v1/admin/bootstrap-tokens` 转发到 token-service 的同名路径。
//!
//! 权限码（`contracts/permissions.yaml`）：
//!   - `TOKEN_BOOTSTRAP_READ`   —— 列表 / 查单条
//!   - `TOKEN_BOOTSTRAP_CREATE` —— 新建
//!   - `TOKEN_BOOTSTRAP_UPDATE` —— rotate / 启停
//!   - `TOKEN_BOOTSTRAP_DELETE` —— 删除
//!
//! ⚠️ user-jwt 的 `aud` 不做强制（`Principal` extractor 用 `None`）——
//! 网关已经验签 + 查黑名单，下游信任网关。真正的门禁是权限码。

use crate::routes::bootstrap_token_admin::{
    ops, BootstrapTokenCreated, BootstrapTokenDto, CreateBootstrapTokenRequest,
    ListBootstrapTokensResponse, RotateBootstrapTokenResponse, SetStatusRequest,
};
use crate::TokenServerState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use digua_authz::perms::perms;
use digua_jwt_ctx::Principal;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

/// 校验 principal 具备指定权限码，否则 403。
fn require_perm(p: &Principal, perm: &str) -> Result<(), (StatusCode, &'static str)> {
    if p.has_permission(perm) {
        Ok(())
    } else {
        tracing::warn!("permission denied: user={} perm={}", p.user_id, perm);
        Err((StatusCode::FORBIDDEN, "permission denied"))
    }
}

// ── DTO ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListQuery {
    /// 按 owner 模糊过滤。
    pub keyword: Option<String>,
    /// `active` | `disabled`；缺省 = 不过滤。
    pub status: Option<String>,
}

/// 轮换请求体（Web UI 用 JSON body 而非 query string）。
#[derive(Debug, Deserialize, ToSchema)]
pub struct RotateRequest {
    /// 旧 token 的宽限期（秒）。0 = 立即失效；>0 = 新旧 token 并存到该时刻。
    #[serde(default)]
    pub grace_period_sec: Option<i64>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /v1/admin/bootstrap-tokens — 列表（不含明文）。
#[utoipa::path(
    get,
    path = "/v1/admin/bootstrap-tokens",
    summary = "List bootstrap tokens (Web UI, requires TOKEN_BOOTSTRAP_READ)",
    params(ListQuery),
    responses(
        (status = 200, body = ListBootstrapTokensResponse),
        (status = 403, description = "permission denied"),
    ),
    security(("bearer_auth" = [])),
    tag = "bootstrap-token"
)]
pub async fn handle_list(
    State(state): State<TokenServerState>,
    principal: Principal,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListBootstrapTokensResponse>, (StatusCode, &'static str)> {
    require_perm(&principal, perms::TOKEN_BOOTSTRAP_READ)?;

    let mut resp = ops::list(&state.db).await?;
    if let Some(kw) = q.keyword.as_deref().filter(|s| !s.is_empty()) {
        let needle = kw.to_lowercase();
        resp.items
            .retain(|i| i.owner.to_lowercase().contains(&needle));
    }
    if let Some(st) = q.status.as_deref().filter(|s| !s.is_empty()) {
        resp.items.retain(|i| i.status == st);
    }
    resp.total = resp.items.len() as i64;
    Ok(Json(resp))
}

/// GET /v1/admin/bootstrap-tokens/{owner} — 查单条（不含明文）。
#[utoipa::path(
    get,
    path = "/v1/admin/bootstrap-tokens/{owner}",
    summary = "Get one bootstrap token metadata (Web UI)",
    params(("owner" = String, Path,)),
    responses(
        (status = 200, body = BootstrapTokenDto),
        (status = 403, description = "permission denied"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "bootstrap-token"
)]
pub async fn handle_get(
    State(state): State<TokenServerState>,
    principal: Principal,
    Path(owner): Path<String>,
) -> Result<Json<BootstrapTokenDto>, (StatusCode, &'static str)> {
    require_perm(&principal, perms::TOKEN_BOOTSTRAP_READ)?;
    Ok(Json(ops::get(&state.db, &owner).await?))
}

/// POST /v1/admin/bootstrap-tokens — 新建（返回明文一次）。
#[utoipa::path(
    post,
    path = "/v1/admin/bootstrap-tokens",
    summary = "Create a bootstrap token (Web UI, returns plaintext once)",
    request_body = CreateBootstrapTokenRequest,
    responses(
        (status = 200, body = BootstrapTokenCreated),
        (status = 400, description = "bad request"),
        (status = 403, description = "permission denied"),
        (status = 409, description = "owner already exists"),
    ),
    security(("bearer_auth" = [])),
    tag = "bootstrap-token"
)]
pub async fn handle_create(
    State(state): State<TokenServerState>,
    principal: Principal,
    Json(req): Json<CreateBootstrapTokenRequest>,
) -> Result<Json<BootstrapTokenCreated>, (StatusCode, &'static str)> {
    require_perm(&principal, perms::TOKEN_BOOTSTRAP_CREATE)?;
    Ok(Json(ops::create(&state.db, &req).await?))
}

/// POST /v1/admin/bootstrap-tokens/{owner}/rotate — 换新 token（返回明文一次）。
#[utoipa::path(
    post,
    path = "/v1/admin/bootstrap-tokens/{owner}/rotate",
    summary = "Rotate a bootstrap token (Web UI, returns new plaintext once)",
    params(("owner" = String, Path,)),
    request_body = RotateRequest,
    responses(
        (status = 200, body = RotateBootstrapTokenResponse),
        (status = 403, description = "permission denied"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "bootstrap-token"
)]
pub async fn handle_rotate(
    State(state): State<TokenServerState>,
    principal: Principal,
    Path(owner): Path<String>,
    Json(req): Json<RotateRequest>,
) -> Result<Json<RotateBootstrapTokenResponse>, (StatusCode, &'static str)> {
    require_perm(&principal, perms::TOKEN_BOOTSTRAP_UPDATE)?;
    Ok(Json(
        ops::rotate(&state.db, &owner, req.grace_period_sec.unwrap_or(0)).await?,
    ))
}

/// PUT /v1/admin/bootstrap-tokens/{owner}/status — 启用 / 禁用。
#[utoipa::path(
    put,
    path = "/v1/admin/bootstrap-tokens/{owner}/status",
    summary = "Enable or disable a bootstrap token (Web UI)",
    params(("owner" = String, Path,)),
    request_body = SetStatusRequest,
    responses(
        (status = 200, body = BootstrapTokenDto),
        (status = 400, description = "bad request"),
        (status = 403, description = "permission denied"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "bootstrap-token"
)]
pub async fn handle_set_status(
    State(state): State<TokenServerState>,
    principal: Principal,
    Path(owner): Path<String>,
    Json(req): Json<SetStatusRequest>,
) -> Result<Json<BootstrapTokenDto>, (StatusCode, &'static str)> {
    require_perm(&principal, perms::TOKEN_BOOTSTRAP_UPDATE)?;
    Ok(Json(ops::set_status(&state.db, &owner, &req.status).await?))
}

/// DELETE /v1/admin/bootstrap-tokens/{owner} — 删除。
#[utoipa::path(
    delete,
    path = "/v1/admin/bootstrap-tokens/{owner}",
    summary = "Delete a bootstrap token (Web UI)",
    params(("owner" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 403, description = "permission denied"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "bootstrap-token"
)]
pub async fn handle_delete(
    State(state): State<TokenServerState>,
    principal: Principal,
    Path(owner): Path<String>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    require_perm(&principal, perms::TOKEN_BOOTSTRAP_DELETE)?;
    ops::delete(&state.db, &owner).await
}

/// `/v1/admin/bootstrap-tokens*` 路由（user-jwt + 权限码）。
///
/// 走 `Principal` extractor（验签 + 解出 permissions），再逐 handler 校验权限码。
pub fn routes(state: crate::TokenServerState) -> axum::Router {
    use axum::routing::{get, post};

    axum::Router::new()
        .route(
            "/v1/admin/bootstrap-tokens",
            get(handle_list).post(handle_create),
        )
        .route(
            "/v1/admin/bootstrap-tokens/{owner}",
            get(handle_get).delete(handle_delete),
        )
        .route(
            "/v1/admin/bootstrap-tokens/{owner}/rotate",
            post(handle_rotate),
        )
        .route(
            "/v1/admin/bootstrap-tokens/{owner}/status",
            axum::routing::put(handle_set_status),
        )
        .with_state(state)
}
