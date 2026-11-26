//! WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01: service JWT audience grants 的
//! **Web UI 用**管理端点。
//!
//! 鉴权：user-jwt（浏览器登录后由 gateway 签发）+ 权限码校验。
//! 与 `/admin/service-jwt-grants*`（service-jwt，给 CI / 运维程序）共用
//! `ops` 业务层（见 `service_jwt_grant_admin.rs`）。
//!
//! 路由前缀 `/v1/admin/*` —— 与 api-gateway 的 `token.router.toml` 约定一致，
//! 网关把 `/v1/admin/service-jwt-grants` 转发到 token-service 同名路径。
//!
//! 权限码（`contracts/permissions.yaml`，运行时由 `Principal::has_permission`
//! 强制执行，不是文档摆设）：
//!   - `TOKEN_SERVICE_JWT_GRANT_READ`   —— 列表 / 查单条
//!   - `TOKEN_SERVICE_JWT_GRANT_CREATE` —— 创建
//!   - `TOKEN_SERVICE_JWT_GRANT_UPDATE` —— 启停
//!   - `TOKEN_SERVICE_JWT_GRANT_DELETE` —— 删除
//!
//! ⚠️ user-jwt 的 `aud` 不做强制（`Principal` extractor 用 `None`）——
//! 网关已经验签 + 查黑名单，下游信任网关。真正的门禁是权限码。
//! 变更类权限拒绝写一行 `denied` 审计（best-effort）。

use crate::models::service_jwt_grant::{Actor, AuditAction};
use crate::routes::service_jwt_grant_admin::{
    ops, request_id_from_headers, CreateServiceJwtGrantRequest, ListGrantsQuery,
    ListServiceJwtGrantsResponse, ServiceJwtGrantDto, SetGrantStatusRequest,
};
use crate::TokenServerState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use digua_authz::perms::perms;
use digua_jwt_ctx::Principal;

/// 权限码 → 审计 action（权限拒绝场景）。
fn perm_action(perm: &str) -> AuditAction {
    match perm {
        perms::TOKEN_SERVICE_JWT_GRANT_CREATE => AuditAction::Create,
        perms::TOKEN_SERVICE_JWT_GRANT_UPDATE => AuditAction::Update,
        perms::TOKEN_SERVICE_JWT_GRANT_DELETE => AuditAction::Delete,
        _ => AuditAction::Update,
    }
}

/// 校验 principal 具备指定权限码，否则 403。
///
/// `write_denied_audit = true` 时（变更类操作）best-effort 记一行 denied 审计；
/// 查询类（READ）权限拒绝只记 tracing 日志，不写审计。
macro_rules! require_perm {
    ($state:expr, $principal:expr, $headers:expr, $perm:expr, $owner:expr, $audience:expr, $audit:expr) => {
        if $principal.has_permission($perm) {
            Ok(())
        } else {
            tracing::warn!(
                user = %$principal.user_id,
                perm = $perm,
                "service-jwt-grant permission denied"
            );
            if $audit {
                let actor = Actor::User {
                    user_id: $principal.user_id.clone(),
                };
                let rid = request_id_from_headers(&$headers);
                if let Err(e) = crate::models::service_jwt_grant::record_audit(
                    &$state.db,
                    &actor,
                    &$owner,
                    &$audience,
                    perm_action($perm),
                    None,
                    None,
                    &rid,
                    "denied",
                    "permission denied",
                )
                .await
                {
                    tracing::error!(error = %e, "grant permission-denied audit write failed");
                }
            }
            Err((StatusCode::FORBIDDEN, "permission denied"))
        }
    };
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /v1/admin/service-jwt-grants — 分页查询。
#[utoipa::path(
    get,
    path = "/v1/admin/service-jwt-grants",
    summary = "List service JWT audience grants (Web UI, requires TOKEN_SERVICE_JWT_GRANT_READ)",
    params(ListGrantsQuery),
    responses(
        (status = 200, body = ListServiceJwtGrantsResponse),
        (status = 400, description = "bad request (paging / status)"),
        (status = 401, description = "missing or invalid user-jwt"),
        (status = 403, description = "permission denied"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_list(
    State(state): State<TokenServerState>,
    principal: Principal,
    Query(q): Query<ListGrantsQuery>,
) -> Result<Json<ListServiceJwtGrantsResponse>, (StatusCode, &'static str)> {
    if !principal.has_permission(perms::TOKEN_SERVICE_JWT_GRANT_READ) {
        tracing::warn!(
            user = %principal.user_id,
            perm = perms::TOKEN_SERVICE_JWT_GRANT_READ,
            "service-jwt-grant permission denied"
        );
        return Err((StatusCode::FORBIDDEN, "permission denied"));
    }
    Ok(Json(ops::list(&state.db, &q).await?))
}

/// GET /v1/admin/service-jwt-grants/{owner}/{audience} — 查单条。
#[utoipa::path(
    get,
    path = "/v1/admin/service-jwt-grants/{owner}/{audience}",
    summary = "Get one service JWT audience grant (Web UI)",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "invalid path params"),
        (status = 401, description = "missing or invalid user-jwt"),
        (status = 403, description = "permission denied"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_get(
    state: State<TokenServerState>,
    principal: Principal,
    headers: HeaderMap,
    Path((owner, audience)): Path<(String, String)>,
) -> Result<Json<ServiceJwtGrantDto>, (StatusCode, &'static str)> {
    require_perm!(
        state, principal, headers,
        perms::TOKEN_SERVICE_JWT_GRANT_READ, owner, audience, false
    )?;
    Ok(Json(ops::get(&state.db, &owner, &audience).await?))
}

/// POST /v1/admin/service-jwt-grants — 创建授权。
#[utoipa::path(
    post,
    path = "/v1/admin/service-jwt-grants",
    summary = "Create a service JWT audience grant (Web UI, requires TOKEN_SERVICE_JWT_GRANT_CREATE)",
    request_body = CreateServiceJwtGrantRequest,
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "invalid owner/audience"),
        (status = 401, description = "missing or invalid user-jwt"),
        (status = 403, description = "permission denied"),
        (status = 409, description = "grant already exists"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_create(
    state: State<TokenServerState>,
    principal: Principal,
    headers: HeaderMap,
    Json(req): Json<CreateServiceJwtGrantRequest>,
) -> Result<Json<ServiceJwtGrantDto>, (StatusCode, &'static str)> {
    let (owner, audience) = (req.owner.clone(), req.audience.clone());
    require_perm!(
        state, principal, headers,
        perms::TOKEN_SERVICE_JWT_GRANT_CREATE, owner, audience, true
    )?;
    let actor = Actor::User {
        user_id: principal.user_id.clone(),
    };
    let request_id = request_id_from_headers(&headers);
    Ok(Json(
        ops::create(&state.db, &req, &actor, &request_id).await?,
    ))
}

/// PUT /v1/admin/service-jwt-grants/{owner}/{audience}/status — 启用 / 禁用。
#[utoipa::path(
    put,
    path = "/v1/admin/service-jwt-grants/{owner}/{audience}/status",
    summary = "Enable or disable a service JWT audience grant (Web UI, requires TOKEN_SERVICE_JWT_GRANT_UPDATE)",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    request_body = SetGrantStatusRequest,
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "bad request (status / path params)"),
        (status = 401, description = "missing or invalid user-jwt"),
        (status = 403, description = "permission denied"),
        (status = 404, description = "not found"),
        (status = 409, description = "system-managed grant is protected"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_set_status(
    state: State<TokenServerState>,
    principal: Principal,
    headers: HeaderMap,
    Path((owner, audience)): Path<(String, String)>,
    Json(req): Json<SetGrantStatusRequest>,
) -> Result<Json<ServiceJwtGrantDto>, (StatusCode, &'static str)> {
    require_perm!(
        state, principal, headers,
        perms::TOKEN_SERVICE_JWT_GRANT_UPDATE, owner, audience, true
    )?;
    let actor = Actor::User {
        user_id: principal.user_id.clone(),
    };
    let request_id = request_id_from_headers(&headers);
    Ok(Json(
        ops::set_status(&state.db, &owner, &audience, &req.status, &actor, &request_id)
            .await?,
    ))
}

/// DELETE /v1/admin/service-jwt-grants/{owner}/{audience} — 删除授权。
#[utoipa::path(
    delete,
    path = "/v1/admin/service-jwt-grants/{owner}/{audience}",
    summary = "Delete a service JWT audience grant (Web UI, requires TOKEN_SERVICE_JWT_GRANT_DELETE)",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 400, description = "invalid path params"),
        (status = 401, description = "missing or invalid user-jwt"),
        (status = 403, description = "permission denied"),
        (status = 404, description = "not found"),
        (status = 409, description = "system-managed grant is protected"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_delete(
    state: State<TokenServerState>,
    principal: Principal,
    headers: HeaderMap,
    Path((owner, audience)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    require_perm!(
        state, principal, headers,
        perms::TOKEN_SERVICE_JWT_GRANT_DELETE, owner, audience, true
    )?;
    let actor = Actor::User {
        user_id: principal.user_id.clone(),
    };
    let request_id = request_id_from_headers(&headers);
    ops::delete(&state.db, &owner, &audience, &actor, &request_id).await
}

/// `/v1/admin/service-jwt-grants*` 路由（user-jwt + 权限码）。
///
/// 走 `Principal` extractor（验签 + 解出 permissions），再逐 handler 校验权限码。
pub fn routes(state: crate::TokenServerState) -> axum::Router {
    use axum::routing::{get, put};

    axum::Router::new()
        .route(
            "/v1/admin/service-jwt-grants",
            get(handle_list).post(handle_create),
        )
        .route(
            "/v1/admin/service-jwt-grants/{owner}/{audience}",
            get(handle_get).delete(handle_delete),
        )
        .route(
            "/v1/admin/service-jwt-grants/{owner}/{audience}/status",
            put(handle_set_status),
        )
        .with_state(state)
}
