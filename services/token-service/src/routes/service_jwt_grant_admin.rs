//! WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01: service JWT audience grants 的管理端点。
//!
//! 两套路由共用同一份业务逻辑（`ops` 模块）：
//!
//! | 路由 | 鉴权 | 用途 |
//! |---|---|---|
//! | `/admin/service-jwt-grants*` | service-jwt（`aud=token-service`） | CI / 运维程序 |
//! | `/v1/admin/service-jwt-grants*`（见 `service_jwt_grant_web.rs`） | user-jwt + 权限码 | Web UI |
//!
//! 与 `/admin/bootstrap-tokens*`（凭据发放面）同级安全：本面控制"谁能给自己
//! 签发哪个 audience 的 service-jwt"，故 service 入口必须验签 + aud 强制
//! `token-service`，普通业务服务不能仅凭自己的业务 JWT 进来。
//!
//! 每个变更操作写一行审计（`token_service_jwt_grant_audit`，append-only）；
//! 审计写入失败 → 500（fail-closed：不留审计的变更视为失败）。审计与变更
//! 非同事务（`db_*!` 宏不支持事务 executor）——审计失败时变更已生效，重试
//! 会得到 409/404 的幂等结果。

use crate::models::service_jwt_grant as grant;
use crate::models::service_jwt_grant::{Actor, CreateOutcome};
use crate::TokenServerState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

fn internal(e: sqlx::Error) -> (StatusCode, &'static str) {
    tracing::error!(error = %e, "service jwt grant admin op failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

/// 分页默认值与上限：size 缺省 20，最大 100（防超大查询）；page 从 1 起。
pub const DEFAULT_PAGE_SIZE: u32 = 20;
pub const MAX_PAGE_SIZE: u32 = 100;

/// 从 `X-Request-Id`（可选）解析请求关联 ID；缺省生成 uuid。
pub fn request_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("X-Request-Id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 64)
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string())
}

// ── DTO ──────────────────────────────────────────────────────────────────────

/// grant 行视图（无任何凭据材料——grant 本身不含 secret）。
#[derive(Debug, Serialize, ToSchema)]
pub struct ServiceJwtGrantDto {
    pub owner: String,
    pub audience: String,
    /// `active` | `disabled`。
    pub status: String,
    /// true = 内置系统授权（管理面禁止改删）。
    pub system_managed: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<grant::GrantRow> for ServiceJwtGrantDto {
    fn from(r: grant::GrantRow) -> Self {
        Self {
            owner: r.owner,
            audience: r.audience,
            status: r.status,
            system_managed: r.system_managed,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// 统一分页返回结构：`{ items, total, page, size }`。
#[derive(Debug, Serialize, ToSchema)]
pub struct ListServiceJwtGrantsResponse {
    pub items: Vec<ServiceJwtGrantDto>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListGrantsQuery {
    /// owner 精确筛选。
    pub owner: Option<String>,
    /// audience 精确筛选。
    pub audience: Option<String>,
    /// `active` | `disabled`；其他值 → 400。
    pub status: Option<String>,
    /// 页码，从 1 起；缺省 1。
    pub page: Option<u32>,
    /// 每页条数（1..=100）；缺省 20。
    pub size: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateServiceJwtGrantRequest {
    pub owner: String,
    pub audience: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetGrantStatusRequest {
    /// `active` | `disabled`。
    pub status: String,
}

// ── 业务逻辑（两套路由共用）────────────────────────────────────────────────

/// 业务操作层：与鉴权无关，供 service-jwt / user-jwt 两套路由调用。
pub mod ops {
    use super::*;

    /// 分页查询（含过滤）。page/size 越界 → 400；status 非法 → 400。
    pub async fn list(
        db: &crate::db::DbPool,
        q: &ListGrantsQuery,
    ) -> Result<ListServiceJwtGrantsResponse, (StatusCode, &'static str)> {
        let page = q.page.unwrap_or(1);
        let size = q.size.unwrap_or(DEFAULT_PAGE_SIZE);
        if page == 0 {
            return Err((StatusCode::BAD_REQUEST, "page must be >= 1"));
        }
        if size == 0 || size > MAX_PAGE_SIZE {
            return Err((StatusCode::BAD_REQUEST, "size must be 1..=100"));
        }
        let status = match q.status.as_deref().filter(|s| !s.is_empty()) {
            None => None,
            Some(raw) => Some(
                grant::parse_status(raw)
                    .ok_or((StatusCode::BAD_REQUEST, "status must be active|disabled"))?,
            ),
        };
        let (rows, total) = grant::list_filtered(
            db,
            q.owner.as_deref().filter(|s| !s.is_empty()),
            q.audience.as_deref().filter(|s| !s.is_empty()),
            status,
            page,
            size,
        )
        .await
        .map_err(internal)?;
        Ok(ListServiceJwtGrantsResponse {
            items: rows.into_iter().map(ServiceJwtGrantDto::from).collect(),
            total,
            page,
            size,
        })
    }

    /// 查单条。
    pub async fn get(
        db: &crate::db::DbPool,
        owner: &str,
        audience: &str,
    ) -> Result<ServiceJwtGrantDto, (StatusCode, &'static str)> {
        validate_pair(owner, audience)?;
        let row = grant::find(db, owner, audience)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "grant not found"))?;
        Ok(ServiceJwtGrantDto::from(row))
    }

    /// 新建 grant（`active`, `system_managed=false`）。重复 → 409。
    pub async fn create(
        db: &crate::db::DbPool,
        req: &CreateServiceJwtGrantRequest,
        actor: &Actor,
        request_id: &str,
    ) -> Result<ServiceJwtGrantDto, (StatusCode, &'static str)> {
        validate_pair(&req.owner, &req.audience)?;
        match grant::create(db, &req.owner, &req.audience).await {
            Ok(CreateOutcome::Inserted) => {}
            Ok(CreateOutcome::Duplicate) => {
                audit_best_effort_denied(
                    db,
                    actor,
                    &req.owner,
                    &req.audience,
                    grant::AuditAction::Create,
                    request_id,
                    "duplicate",
                )
                .await;
                return Err((StatusCode::CONFLICT, "grant already exists"));
            }
            Err(e) => return Err(internal(e)),
        }
        write_audit(
            db,
            actor,
            &req.owner,
            &req.audience,
            grant::AuditAction::Create,
            None,
            Some(grant::STATUS_ACTIVE),
            request_id,
            "success",
            "grant created",
        )
        .await?;
        let row = grant::find(db, &req.owner, &req.audience)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "grant not found"))?;
        Ok(ServiceJwtGrantDto::from(row))
    }

    /// 启用 / 禁用。非法状态 400；行不存在 404；`system_managed` 行 409。
    ///
    /// 条件 UPDATE（`AND status <> ?`）防并发覆盖：两个并发 disable 只有一个
    /// 真正写库，另一个拿到 `affected=0` 后按"已是目标状态"幂等返回。
    pub async fn set_status(
        db: &crate::db::DbPool,
        owner: &str,
        audience: &str,
        status: &str,
        actor: &Actor,
        request_id: &str,
    ) -> Result<ServiceJwtGrantDto, (StatusCode, &'static str)> {
        validate_pair(owner, audience)?;
        let target = grant::parse_status(status)
            .ok_or((StatusCode::BAD_REQUEST, "status must be active|disabled"))?;
        let before = match grant::find(db, owner, audience).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                audit_best_effort_denied(
                    db,
                    actor,
                    owner,
                    audience,
                    status_action(target),
                    request_id,
                    "not found",
                )
                .await;
                return Err((StatusCode::NOT_FOUND, "grant not found"));
            }
            Err(e) => return Err(internal(e)),
        };
        if before.system_managed {
            write_audit(
                db,
                actor,
                owner,
                audience,
                status_action(target),
                Some(&before.status),
                Some(&before.status),
                request_id,
                "denied",
                "system-managed grant is protected",
            )
            .await?;
            return Err((
                StatusCode::CONFLICT,
                "system-managed grant cannot be modified",
            ));
        }
        if before.status == target {
            // 幂等：已处于目标状态，直接返回当前行（不写"无操作"审计）。
            return Ok(ServiceJwtGrantDto::from(before));
        }
        let affected = grant::set_status(db, owner, audience, target)
            .await
            .map_err(internal)?;
        if affected == 0 {
            // 并发下被别人先改到目标状态（或行被删）。重查一次判定语义。
            let now_row = grant::find(db, owner, audience)
                .await
                .map_err(internal)?;
            match now_row {
                None => {
                    audit_best_effort_denied(
                        db,
                        actor,
                        owner,
                        audience,
                        status_action(target),
                        request_id,
                        "not found",
                    )
                    .await;
                    return Err((StatusCode::NOT_FOUND, "grant not found"));
                }
                Some(r) => return Ok(ServiceJwtGrantDto::from(r)),
            }
        }
        write_audit(
            db,
            actor,
            owner,
            audience,
            status_action(target),
            Some(&before.status),
            Some(target),
            request_id,
            "success",
            "status updated",
        )
        .await?;
        let row = grant::find(db, owner, audience)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "grant not found"))?;
        Ok(ServiceJwtGrantDto::from(row))
    }

    /// 删除 grant。行不存在/已删 404；`system_managed` 行 409（系统授权保护）。
    pub async fn delete(
        db: &crate::db::DbPool,
        owner: &str,
        audience: &str,
        actor: &Actor,
        request_id: &str,
    ) -> Result<StatusCode, (StatusCode, &'static str)> {
        validate_pair(owner, audience)?;
        let before = match grant::find(db, owner, audience).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                audit_best_effort_denied(
                    db,
                    actor,
                    owner,
                    audience,
                    grant::AuditAction::Delete,
                    request_id,
                    "not found",
                )
                .await;
                return Err((StatusCode::NOT_FOUND, "grant not found"));
            }
            Err(e) => return Err(internal(e)),
        };
        if before.system_managed {
            write_audit(
                db,
                actor,
                owner,
                audience,
                grant::AuditAction::Delete,
                Some(&before.status),
                Some(&before.status),
                request_id,
                "denied",
                "system-managed grant is protected",
            )
            .await?;
            return Err((
                StatusCode::CONFLICT,
                "system-managed grant cannot be deleted",
            ));
        }
        let affected = grant::delete(db, owner, audience)
            .await
            .map_err(internal)?;
        if affected == 0 {
            audit_best_effort_denied(
                db,
                actor,
                owner,
                audience,
                grant::AuditAction::Delete,
                request_id,
                "not found",
            )
            .await;
            return Err((StatusCode::NOT_FOUND, "grant not found"));
        }
        write_audit(
            db,
            actor,
            owner,
            audience,
            grant::AuditAction::Delete,
            Some(&before.status),
            None,
            request_id,
            "success",
            "grant deleted",
        )
        .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    // ── 内部 helper ──────────────────────────────────────────────────────

    fn validate_pair(
        owner: &str,
        audience: &str,
    ) -> Result<(), (StatusCode, &'static str)> {
        grant::validate_name(owner)
            .map_err(|e| (StatusCode::BAD_REQUEST, owner_err(e)))?;
        grant::validate_name(audience)
            .map_err(|e| (StatusCode::BAD_REQUEST, audience_err(e)))?;
        Ok(())
    }

    /// 静态文案（错误响应不携带动态内容，避免任何反射注入面）。
    fn owner_err(_: &'static str) -> &'static str {
        "invalid owner: length 1..=64, [a-zA-Z0-9_-]"
    }
    fn audience_err(_: &'static str) -> &'static str {
        "invalid audience: length 1..=64, [a-zA-Z0-9_-]"
    }

    fn status_action(target: &'static str) -> grant::AuditAction {
        if target == grant::STATUS_ACTIVE {
            grant::AuditAction::Enable
        } else {
            grant::AuditAction::Disable
        }
    }

    /// 404 时的 denied 审计由 `audit_best_effort_denied` 补记（见 set_status/delete）。

    /// best-effort denied 审计：失败只 log（denied 响应本身不受审计可用性影响）。
    async fn audit_best_effort_denied(
        db: &crate::db::DbPool,
        actor: &Actor,
        owner: &str,
        audience: &str,
        action: grant::AuditAction,
        request_id: &str,
        detail: &str,
    ) {
        if let Err(e) = grant::record_audit(
            db, actor, owner, audience, action, None, None, request_id, "denied", detail,
        )
        .await
        {
            tracing::error!(error = %e, "grant denied-audit write failed");
        }
    }

    /// 成功路径审计：fail-closed——写入失败映射 500（调用方把变更视为失败）。
    #[allow(clippy::too_many_arguments)]
    async fn write_audit(
        db: &crate::db::DbPool,
        actor: &Actor,
        owner: &str,
        audience: &str,
        action: grant::AuditAction,
        before: Option<&str>,
        after: Option<&str>,
        request_id: &str,
        result: &str,
        detail: &str,
    ) -> Result<(), (StatusCode, &'static str)> {
        grant::record_audit(
            db, actor, owner, audience, action, before, after, request_id, result, detail,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "grant audit write failed; operation NOT acknowledged");
            (StatusCode::INTERNAL_SERVER_ERROR, "audit write failed")
        })
    }
}

// ── service-jwt 路由（CI / 运维程序）─────────────────────────────────────────

/// GET /admin/service-jwt-grants — 分页查询。
#[utoipa::path(
    get,
    path = "/admin/service-jwt-grants",
    summary = "List service JWT audience grants (requires service-jwt, aud=token-service)",
    params(ListGrantsQuery),
    responses(
        (status = 200, body = ListServiceJwtGrantsResponse),
        (status = 400, description = "bad request (paging / status)"),
        (status = 401, description = "missing or invalid service-jwt"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_list(
    State(state): State<TokenServerState>,
    Query(q): Query<ListGrantsQuery>,
) -> Result<Json<ListServiceJwtGrantsResponse>, (StatusCode, &'static str)> {
    Ok(Json(ops::list(&state.db, &q).await?))
}

/// GET /admin/service-jwt-grants/{owner}/{audience} — 查单条。
#[utoipa::path(
    get,
    path = "/admin/service-jwt-grants/{owner}/{audience}",
    summary = "Get one service JWT audience grant",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "invalid path params"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_get(
    State(state): State<TokenServerState>,
    Path((owner, audience)): Path<(String, String)>,
) -> Result<Json<ServiceJwtGrantDto>, (StatusCode, &'static str)> {
    Ok(Json(ops::get(&state.db, &owner, &audience).await?))
}

/// POST /admin/service-jwt-grants — 创建授权。
#[utoipa::path(
    post,
    path = "/admin/service-jwt-grants",
    summary = "Create a service JWT audience grant",
    request_body = CreateServiceJwtGrantRequest,
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "invalid owner/audience"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 409, description = "grant already exists"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_create(
    State(state): State<TokenServerState>,
    headers: HeaderMap,
    principal: digua_jwt_ctx::Principal,
    Json(req): Json<CreateServiceJwtGrantRequest>,
) -> Result<Json<ServiceJwtGrantDto>, (StatusCode, &'static str)> {
    let actor = Actor::Service {
        subject: principal.user_id.clone(),
    };
    let request_id = request_id_from_headers(&headers);
    Ok(Json(
        ops::create(&state.db, &req, &actor, &request_id).await?,
    ))
}

/// PUT /admin/service-jwt-grants/{owner}/{audience}/status — 启用 / 禁用。
#[utoipa::path(
    put,
    path = "/admin/service-jwt-grants/{owner}/{audience}/status",
    summary = "Enable or disable a service JWT audience grant",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    request_body = SetGrantStatusRequest,
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "bad request (status / path params)"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 404, description = "not found"),
        (status = 409, description = "system-managed grant is protected"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_set_status(
    State(state): State<TokenServerState>,
    headers: HeaderMap,
    principal: digua_jwt_ctx::Principal,
    Path((owner, audience)): Path<(String, String)>,
    Json(req): Json<SetGrantStatusRequest>,
) -> Result<Json<ServiceJwtGrantDto>, (StatusCode, &'static str)> {
    let actor = Actor::Service {
        subject: principal.user_id.clone(),
    };
    let request_id = request_id_from_headers(&headers);
    Ok(Json(
        ops::set_status(&state.db, &owner, &audience, &req.status, &actor, &request_id)
            .await?,
    ))
}

/// DELETE /admin/service-jwt-grants/{owner}/{audience} — 删除授权。
#[utoipa::path(
    delete,
    path = "/admin/service-jwt-grants/{owner}/{audience}",
    summary = "Delete a service JWT audience grant",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 400, description = "invalid path params"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 404, description = "not found"),
        (status = 409, description = "system-managed grant is protected"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_delete(
    State(state): State<TokenServerState>,
    headers: HeaderMap,
    principal: digua_jwt_ctx::Principal,
    Path((owner, audience)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let actor = Actor::Service {
        subject: principal.user_id.clone(),
    };
    let request_id = request_id_from_headers(&headers);
    ops::delete(&state.db, &owner, &audience, &actor, &request_id).await
}

/// `/admin/service-jwt-grants*` 路由（带 service-jwt 鉴权，aud=token-service）。
pub fn routes(state: crate::TokenServerState) -> axum::Router {
    use axum::routing::{get, put};

    axum::Router::new()
        .route(
            "/admin/service-jwt-grants",
            get(handle_list).post(handle_create),
        )
        .route(
            "/admin/service-jwt-grants/{owner}/{audience}",
            get(handle_get).delete(handle_delete),
        )
        .route(
            "/admin/service-jwt-grants/{owner}/{audience}/status",
            put(handle_set_status),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<crate::TokenServerState>(
                "token-service",
            ),
        ))
        .with_state(state)
}
