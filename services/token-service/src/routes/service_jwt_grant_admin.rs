//! WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01: service JWT audience grants 的管理端点。
//!
//! 上游 WP-LMMS-SERVICE-JWT-AUDIENCE-02 建立了运行时授权检查（is_allowed）
//! 与 code-owned 系统 seed；本模块补齐管理面（CRUD + 审计）。
//!
//! 两套路由共用同一份业务逻辑（`ops` 模块）：
//!
//! | 路由 | 鉴权 | 用途 |
//! |---|---|---|
//! | `/admin/service-jwt-grants*` | service-jwt（`aud=token-service`）+ **grant 管理员清单** | CI / 运维程序 |
//! | `/v1/admin/service-jwt-grants*`（见 `service_jwt_grant_web.rs`） | user-jwt + 权限码 | Web UI |
//!
//! ⚠️ P1-01（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-02）：service 入口**不再**
//! 仅凭 `aud=token-service` 放行——`aud` 中间件只证明"有权访问
//! token-service"，管理 grant 是独立的更高级权限。每个 handler 在任何业务
//! 读写之前执行 [`require_grant_admin`]：验签后的 JWT `sub`（canonical
//! bootstrap owner，客户端不可伪造）必须命中 `TOKEN_SERVICE_JWT_GRANT_ADMINS`
//! 部署清单（空清单 = fail-closed 全拒）。清单缺失/不含该主体 → 403，
//! 变更类另记一行 best-effort denied 审计。请求体 / 路径参数
//!（owner、audience）只是操作目标，不参与权限判定。Web UI 的 user-jwt +
//! 权限码入口不受影响。
//!
//! P1-02：每个变更操作与其 success 审计行**同一事务提交**
//!（`DbPool::begin` + `db_tx_*!` 宏）：审计失败 → 业务变更整体回滚 → 500，
//! 数据库不留"已变更但无审计"的状态。读走 `FOR UPDATE` 行锁闭合
//! 先查后改窗口；system_managed 保护仍由条件 UPDATE / DELETE 在 SQL 内
//! 原子兜底。denied / 404 / 权限拒绝路径无业务变更，审计 best-effort
//!（失败只 log），语义与 ADMIN-01 一致。

use crate::models::service_jwt_grant as grant;
use crate::models::service_jwt_grant::{Actor, CreateOutcome};
use crate::TokenServerState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use digua_common_db::Transaction;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

fn internal(e: sqlx::Error) -> (StatusCode, &'static str) {
    tracing::error!(error = %e, "service jwt grant admin op failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

/// 显式回滚事务（错误仅 log——drop 未提交事务时 sqlx 连接层还会兜底回滚）。
async fn rollback_quiet(tx: Transaction<'_>) {
    if let Err(rb) = tx.rollback().await {
        tracing::error!(error = %rb, "grant tx explicit rollback failed; relying on drop-rollback");
    }
}

/// 回滚事务并把业务 SQL 错误映射 500。
async fn rollback_internal(tx: Transaction<'_>, e: sqlx::Error) -> (StatusCode, &'static str) {
    rollback_quiet(tx).await;
    internal(e)
}

/// P1-01：service-jwt 管理面权限门。**必须在任何业务读写 / 审计之前调用**。
///
/// - 身份来源：验签后的 service-jwt `sub`（token-service 签发时从 DB
///   bootstrap owner 写入，RS256 + aud 中间件保护，客户端不可伪造）；
/// - 权限来源：`TOKEN_SERVICE_JWT_GRANT_ADMINS` 部署清单（空 = 全拒）；
/// - 请求体 / 路径参数（owner、audience、status）只是操作目标，
///   **不参与**判定，改参数无法越权。
fn require_grant_admin(
    state: &TokenServerState,
    principal: &digua_jwt_ctx::Principal,
) -> Result<(), (StatusCode, &'static str)> {
    if state.is_service_jwt_grant_admin(&principal.user_id) {
        return Ok(());
    }
    tracing::warn!(
        subject = %principal.user_id,
        "service-jwt grant admin denied: subject not in TOKEN_SERVICE_JWT_GRANT_ADMINS"
    );
    Err((StatusCode::FORBIDDEN, "grant admin permission denied"))
}

/// 分页默认值与上限：size 缺省 20，最大 100（防超大查询）；page 从 1 起。
pub const DEFAULT_PAGE_SIZE: u32 = 20;
pub const MAX_PAGE_SIZE: u32 = 100;

/// 从 `X-Request-Id`（可选）解析请求关联 ID；不可用或缺省时服务端生成新 UUID。
///
/// 信任边界（R1-01 安全修复）：客户端值**只接受严格 UUID 格式**——内容由
/// UUID 语法完全约束，长度 / 字符集 / 结构都不受客户端控制，无法把自由
/// 文本（凭据、bootstrap secret、注入串）带进审计表。解析成功后统一规范
/// 化为标准 hyphenated 小写再落库；空白、超长、非法格式、非字符串值一律
/// 回退到服务端生成的新 UUID。非法 Request-ID 不导致管理请求失败（仍保持
/// 正常请求的关联能力）。
pub fn request_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("X-Request-Id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| uuid::Uuid::parse_str(s.trim()).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
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

impl From<grant::ServiceJwtGrantRow> for ServiceJwtGrantDto {
    fn from(r: grant::ServiceJwtGrantRow) -> Self {
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
        let row = grant::find_by_owner_and_audience(db, owner, audience)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "grant not found"))?;
        Ok(ServiceJwtGrantDto::from(row))
    }

    /// 新建 grant（`active`, `system_managed=false`）。重复 → 409。
    ///
    /// P1-02：INSERT grant 与 success 审计**同事务**。审计失败 → 整体回滚
    ///（grant 不存在）→ 500；并发 PK 冲突 → 回滚 → 409。
    pub async fn create(
        db: &crate::db::DbPool,
        req: &CreateServiceJwtGrantRequest,
        actor: &Actor,
        request_id: &str,
    ) -> Result<ServiceJwtGrantDto, (StatusCode, &'static str)> {
        validate_pair(&req.owner, &req.audience)?;
        let mut tx = db.begin().await.map_err(internal)?;
        let outcome = match grant::create_grant_tx(&mut tx, &req.owner, &req.audience).await {
            Ok(o) => o,
            Err(e) => return Err(rollback_internal(tx, e).await),
        };
        if outcome == CreateOutcome::Duplicate {
            // 已存在（含并发双插判负）：回滚后 best-effort 记 denied 审计。
            rollback_quiet(tx).await;
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
        if let Err(e) = grant::record_audit_tx(
            &mut tx,
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
        .await
        {
            tracing::error!(error = %e, "grant audit write failed; rolling back grant INSERT");
            rollback_quiet(tx).await;
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "audit write failed"));
        }
        tx.commit().await.map_err(internal)?;
        let row = grant::find_by_owner_and_audience(db, &req.owner, &req.audience)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "grant not found"))?;
        Ok(ServiceJwtGrantDto::from(row))
    }

    /// 启用 / 禁用。非法状态 400；行不存在 404；`system_managed` 行 409。
    ///
    /// P1-02：状态变更与 success 审计**同事务**（审计失败 → 回滚 → 500，
    /// 状态保持原值）。目标行先 `SELECT ... FOR UPDATE` 行锁锁定——
    /// before 判定（不存在 / system_managed / 已是目标状态）在锁下完成，
    /// 并发请求在锁上串行化，不存在锁下 `affected=0` 的合法路径。
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
        let mut tx = db.begin().await.map_err(internal)?;
        let before =
            match grant::find_by_owner_and_audience_for_update(&mut tx, owner, audience).await {
                Ok(Some(row)) => row,
                Ok(None) => {
                    rollback_quiet(tx).await;
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
                Err(e) => return Err(rollback_internal(tx, e).await),
            };
        if before.system_managed {
            // 行锁下判定：无并发窗口。回滚（无业务变更）后 fail-closed 记
            // denied 审计（写失败 → 500，与 ADMIN-01 语义一致）。
            rollback_quiet(tx).await;
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
            // 幂等：已处于目标状态，无业务变更、无审计，直接返回当前行。
            rollback_quiet(tx).await;
            return Ok(ServiceJwtGrantDto::from(before));
        }
        let affected = match grant::set_grant_status_tx(&mut tx, owner, audience, target).await {
            Ok(n) => n,
            Err(e) => return Err(rollback_internal(tx, e).await),
        };
        if affected == 0 {
            // 锁下不可达（before 已排除 system_managed / 同状态）——
            // 视为内部一致性异常，fail-closed 回滚，不做幂等伪装。
            tracing::error!("conditional grant UPDATE missed under FOR UPDATE lock; rolling back");
            rollback_quiet(tx).await;
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "internal error"));
        }
        if let Err(e) = grant::record_audit_tx(
            &mut tx,
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
        .await
        {
            tracing::error!(error = %e, "grant audit write failed; rolling back grant UPDATE");
            rollback_quiet(tx).await;
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "audit write failed"));
        }
        tx.commit().await.map_err(internal)?;
        let row = grant::find_by_owner_and_audience(db, owner, audience)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "grant not found"))?;
        Ok(ServiceJwtGrantDto::from(row))
    }

    /// 删除 grant。行不存在/已删 404；`system_managed` 行 409（系统授权保护）。
    ///
    /// P1-02：DELETE 与 success 审计**同事务**（审计失败 → 回滚 → 500，
    /// grant 保留）。
    pub async fn delete(
        db: &crate::db::DbPool,
        owner: &str,
        audience: &str,
        actor: &Actor,
        request_id: &str,
    ) -> Result<StatusCode, (StatusCode, &'static str)> {
        validate_pair(owner, audience)?;
        let mut tx = db.begin().await.map_err(internal)?;
        let before =
            match grant::find_by_owner_and_audience_for_update(&mut tx, owner, audience).await {
                Ok(Some(row)) => row,
                Ok(None) => {
                    rollback_quiet(tx).await;
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
                Err(e) => return Err(rollback_internal(tx, e).await),
            };
        if before.system_managed {
            rollback_quiet(tx).await;
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
        let affected = match grant::delete_grant_tx(&mut tx, owner, audience).await {
            Ok(n) => n,
            Err(e) => return Err(rollback_internal(tx, e).await),
        };
        if affected == 0 {
            tracing::error!("conditional grant DELETE missed under FOR UPDATE lock; rolling back");
            rollback_quiet(tx).await;
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "internal error"));
        }
        if let Err(e) = grant::record_audit_tx(
            &mut tx,
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
        .await
        {
            tracing::error!(error = %e, "grant audit write failed; rolling back grant DELETE");
            rollback_quiet(tx).await;
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "audit write failed"));
        }
        tx.commit().await.map_err(internal)?;
        Ok(StatusCode::NO_CONTENT)
    }

    // ── 内部 helper ──────────────────────────────────────────────────────

    fn validate_pair(owner: &str, audience: &str) -> Result<(), (StatusCode, &'static str)> {
        grant::validate_service_identity(owner)
            .map_err(|e| (StatusCode::BAD_REQUEST, owner_err(&e)))?;
        grant::validate_service_identity(audience)
            .map_err(|e| (StatusCode::BAD_REQUEST, audience_err(&e)))?;
        Ok(())
    }

    /// 静态文案（错误响应不携带动态内容，避免任何反射注入面）；
    /// 上游校验器的具体原因不外泄。
    fn owner_err(_: &str) -> &'static str {
        "invalid owner: length 1..=64, [a-zA-Z0-9_-]"
    }
    fn audience_err(_: &str) -> &'static str {
        "invalid audience: length 1..=64, [a-zA-Z0-9_-]"
    }

    fn status_action(target: &'static str) -> grant::AuditAction {
        if target == grant::STATUS_ACTIVE {
            grant::AuditAction::Enable
        } else {
            grant::AuditAction::Disable
        }
    }

    /// best-effort denied 审计：失败只 log（denied 响应本身不受审计可用性影响）。
    /// 供 ops 内部与权限拒绝路径（handle_* 的 P1-01 门）共用。
    pub(crate) async fn audit_best_effort_denied(
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
    summary = "List service JWT audience grants (service-jwt aud=token-service + grant admin list)",
    params(ListGrantsQuery),
    responses(
        (status = 200, body = ListServiceJwtGrantsResponse),
        (status = 400, description = "bad request (paging / status)"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 403, description = "subject not in the grant admin list (TOKEN_SERVICE_JWT_GRANT_ADMINS)"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_list(
    State(state): State<TokenServerState>,
    principal: digua_jwt_ctx::Principal,
    Query(q): Query<ListGrantsQuery>,
) -> Result<Json<ListServiceJwtGrantsResponse>, (StatusCode, &'static str)> {
    require_grant_admin(&state, &principal)?;
    Ok(Json(ops::list(&state.db, &q).await?))
}

/// GET /admin/service-jwt-grants/{owner}/{audience} — 查单条。
#[utoipa::path(
    get,
    path = "/admin/service-jwt-grants/{owner}/{audience}",
    summary = "Get one service JWT audience grant (requires grant admin)",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "invalid path params"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 403, description = "subject not in the grant admin list"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "service-jwt-grant"
)]
pub async fn handle_get(
    State(state): State<TokenServerState>,
    principal: digua_jwt_ctx::Principal,
    Path((owner, audience)): Path<(String, String)>,
) -> Result<Json<ServiceJwtGrantDto>, (StatusCode, &'static str)> {
    require_grant_admin(&state, &principal)?;
    Ok(Json(ops::get(&state.db, &owner, &audience).await?))
}

/// POST /admin/service-jwt-grants — 创建授权。
#[utoipa::path(
    post,
    path = "/admin/service-jwt-grants",
    summary = "Create a service JWT audience grant (requires grant admin)",
    request_body = CreateServiceJwtGrantRequest,
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "invalid owner/audience"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 403, description = "subject not in the grant admin list"),
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
    if let Err(denied) = require_grant_admin(&state, &principal) {
        // 变更类权限拒绝：best-effort 记 denied 审计（owner/audience 仅作为
        // 被拒目标记录，不参与权限判定——改参数无法越权）。
        ops::audit_best_effort_denied(
            &state.db,
            &actor,
            &req.owner,
            &req.audience,
            grant::AuditAction::Create,
            &request_id,
            "permission denied",
        )
        .await;
        return Err(denied);
    }
    Ok(Json(
        ops::create(&state.db, &req, &actor, &request_id).await?,
    ))
}

/// PUT /admin/service-jwt-grants/{owner}/{audience}/status — 启用 / 禁用。
#[utoipa::path(
    put,
    path = "/admin/service-jwt-grants/{owner}/{audience}/status",
    summary = "Enable or disable a service JWT audience grant (requires grant admin)",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    request_body = SetGrantStatusRequest,
    responses(
        (status = 200, body = ServiceJwtGrantDto),
        (status = 400, description = "bad request (status / path params)"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 403, description = "subject not in the grant admin list"),
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
    if let Err(denied) = require_grant_admin(&state, &principal) {
        ops::audit_best_effort_denied(
            &state.db,
            &actor,
            &owner,
            &audience,
            grant::AuditAction::Update,
            &request_id,
            "permission denied",
        )
        .await;
        return Err(denied);
    }
    Ok(Json(
        ops::set_status(
            &state.db,
            &owner,
            &audience,
            &req.status,
            &actor,
            &request_id,
        )
        .await?,
    ))
}

/// DELETE /admin/service-jwt-grants/{owner}/{audience} — 删除授权。
#[utoipa::path(
    delete,
    path = "/admin/service-jwt-grants/{owner}/{audience}",
    summary = "Delete a service JWT audience grant (requires grant admin)",
    params(("owner" = String, Path,), ("audience" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 400, description = "invalid path params"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 403, description = "subject not in the grant admin list"),
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
    if let Err(denied) = require_grant_admin(&state, &principal) {
        ops::audit_best_effort_denied(
            &state.db,
            &actor,
            &owner,
            &audience,
            grant::AuditAction::Delete,
            &request_id,
            "permission denied",
        )
        .await;
        return Err(denied);
    }
    ops::delete(&state.db, &owner, &audience, &actor, &request_id).await
}

/// `/admin/service-jwt-grants*` 路由（service-jwt 验签 + aud=token-service
/// 中间件，handler 内再过 grant 管理员清单门——P1-01 双层防线）。
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

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("X-Request-Id", value.parse().expect("valid header value"));
        h
    }

    #[test]
    fn valid_uuid_is_accepted_and_normalized() {
        let id = request_id_from_headers(&headers_with("0b4f9c5a-1f2e-3d4c-5b6a-7c8d9e0f1a2b"));
        assert_eq!(id, "0b4f9c5a-1f2e-3d4c-5b6a-7c8d9e0f1a2b");
        // simple / braced / urn 变体解析为同一 UUID，统一规范化为 hyphenated
        assert_eq!(
            request_id_from_headers(&headers_with("0b4f9c5a1f2e3d4c5b6a7c8d9e0f1a2b")),
            "0b4f9c5a-1f2e-3d4c-5b6a-7c8d9e0f1a2b"
        );
        assert_eq!(
            request_id_from_headers(&headers_with("{0B4F9C5A-1F2E-3D4C-5B6A-7C8D9E0F1A2B}")),
            "0b4f9c5a-1f2e-3d4c-5b6a-7c8d9e0f1a2b"
        );
    }

    #[test]
    fn invalid_values_are_replaced_by_server_uuid() {
        let server = request_id_from_headers(&HeaderMap::new());
        assert!(uuid::Uuid::parse_str(&server).is_ok());

        for bad in [
            "",                                                 // 空串
            "   ",                                              // 纯空白
            "not-a-uuid",                                       // 非法格式
            "abc'; DROP TABLE token_service_jwt_grant_audit--", // 注入串
            "Bearer eyJhbGciOiJIUzI1NiJ9.e30.sig",              // Authorization 风格内容
            "bootstrap-secret-ak-7f3c9e1d",                     // 凭据风格内容
            "a".repeat(65).as_str(),                            // 超长（>64）
            "0b4f9c5a-1f2e-3d4c-5b6a-7c8d9e0f1a2",              // 缺一位 hex
        ] {
            let id = request_id_from_headers(&headers_with(bad));
            assert!(
                uuid::Uuid::parse_str(&id).is_ok(),
                "invalid value must be replaced by a server-side uuid: {bad:?}"
            );
            assert_ne!(id, bad);
        }
        // 空白包裹的合法 UUID：trim 后仍可解析（不误伤正常请求）
        let id = request_id_from_headers(&headers_with("  0b4f9c5a-1f2e-3d4c-5b6a-7c8d9e0f1a2b  "));
        assert_eq!(id, "0b4f9c5a-1f2e-3d4c-5b6a-7c8d9e0f1a2b");
    }

    #[test]
    fn server_fallback_is_always_a_uuid() {
        for _ in 0..16 {
            assert!(uuid::Uuid::parse_str(&request_id_from_headers(&HeaderMap::new())).is_ok());
        }
    }
}
