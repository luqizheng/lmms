//! Phase 3: `token_bootstrap_tokens` 的管理端点。
//!
//! 两套路由共用同一份业务逻辑（`ops` 模块）：
//!
//! | 路由 | 鉴权 | 用途 |
//! |---|---|---|
//! | `/admin/bootstrap-tokens*` | service-jwt（`aud=token-service`） | CI / 运维脚本 |
//! | `/v1/admin/bootstrap-tokens*` | user-jwt + 权限码 | Web UI（浏览器） |
//!
//! ⚠️ 列表与查询**不返回明文 token**（当前实现里 `token_hash` 列存的就是明文，
//! 因此 DTO 里刻意不带该字段）。create / rotate 会在响应里返回一次明文，
//! 调用方需自行安全保存——明文只在那一刻可见，DB 里没有第二份。

use crate::models::bootstrap_token as bt;
use crate::TokenServerState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

fn internal(e: sqlx::Error) -> (StatusCode, &'static str) {
    tracing::error!(error = %e, "bootstrap token admin op failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

/// owner 的合法字符集：字母 / 数字 / `-` / `_`。
///
/// 排除 `#`（宽限期遗留行用的后缀）与空白，避免 owner 撞上内部行或注入 SQL 片段。
pub fn validate_owner(owner: &str) -> Result<(), (StatusCode, &'static str)> {
    if owner.is_empty() || owner.len() > 64 {
        return Err((StatusCode::BAD_REQUEST, "owner length must be 1..=64"));
    }
    if !owner
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "owner may only contain [a-zA-Z0-9_-]",
        ));
    }
    Ok(())
}

// ── DTO ──────────────────────────────────────────────────────────────────────

/// 列表项：刻意**不含** token 明文。
#[derive(Debug, Serialize, ToSchema)]
pub struct BootstrapTokenDto {
    pub owner: String,
    pub status: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// true 表示这是 rotate 宽限期遗留的旧 token 行。
    pub is_grace_row: bool,
}

impl From<bt::BootstrapTokenRow> for BootstrapTokenDto {
    fn from(r: bt::BootstrapTokenRow) -> Self {
        Self {
            is_grace_row: r.owner.ends_with("#old"),
            owner: r.owner,
            status: r.status,
            expires_at: r.expires_at,
            last_used_at: r.last_used_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListBootstrapTokensResponse {
    pub items: Vec<BootstrapTokenDto>,
    pub total: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateBootstrapTokenRequest {
    /// 服务名（`iam-service` 等），JWT 的 `sub` 即此值。
    pub owner: String,
    /// 过期时间（RFC3339）；缺省 = 不过期。
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BootstrapTokenCreated {
    pub owner: String,
    /// 明文 token——**仅此一次可见**，调用方需立刻保存。
    pub token: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct RotateQuery {
    /// 旧 token 的宽限期（秒）。0 = 立即失效；>0 = 新旧 token 并存到该时刻。
    #[serde(default)]
    pub grace_period_sec: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RotateBootstrapTokenResponse {
    pub owner: String,
    /// 新 token 明文——**仅此一次可见**。
    pub token: String,
    /// 旧 token 的失效时间；None = 旧 token 已立即失效。
    pub old_token_expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetStatusRequest {
    /// `active` | `disabled`。
    pub status: String,
}

// ── 业务逻辑（两套路由共用）────────────────────────────────────────────────

/// 业务操作层：与鉴权无关，供 service-jwt / user-jwt 两套路由调用。
pub mod ops {
    use super::*;

    /// 列出全部 bootstrap token（不含明文）。
    pub async fn list(
        db: &crate::db::DbPool,
    ) -> Result<ListBootstrapTokensResponse, (StatusCode, &'static str)> {
        let rows = bt::list_all(db).await.map_err(internal)?;
        let total = rows.len() as i64;
        Ok(ListBootstrapTokensResponse {
            items: rows.into_iter().map(BootstrapTokenDto::from).collect(),
            total,
        })
    }

    /// 查单条（不含明文）。
    pub async fn get(
        db: &crate::db::DbPool,
        owner: &str,
    ) -> Result<BootstrapTokenDto, (StatusCode, &'static str)> {
        validate_owner(owner)?;
        let row = bt::find_by_owner(db, owner)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "owner not found"))?;
        Ok(BootstrapTokenDto::from(row))
    }

    /// 新建（返回明文一次）。
    pub async fn create(
        db: &crate::db::DbPool,
        req: &CreateBootstrapTokenRequest,
    ) -> Result<BootstrapTokenCreated, (StatusCode, &'static str)> {
        validate_owner(&req.owner)?;
        let expires_at = match req.expires_at.as_deref() {
            None => None,
            Some(raw) => Some(
                chrono::DateTime::parse_from_rfc3339(raw)
                    .map_err(|_| (StatusCode::BAD_REQUEST, "invalid expires_at"))?
                    .with_timezone(&chrono::Utc),
            ),
        };
        let plain = bt::generate_token();
        let created = bt::create(db, &req.owner, &plain, expires_at)
            .await
            .map_err(internal)?;
        if !created {
            return Err((StatusCode::CONFLICT, "owner already exists"));
        }
        tracing::info!("created bootstrap token owner={}", req.owner);
        Ok(BootstrapTokenCreated {
            owner: req.owner.clone(),
            token: plain,
        })
    }

    /// 换新 token（返回明文一次）。
    pub async fn rotate(
        db: &crate::db::DbPool,
        owner: &str,
        grace_period_sec: i64,
    ) -> Result<RotateBootstrapTokenResponse, (StatusCode, &'static str)> {
        validate_owner(owner)?;
        let grace = grace_period_sec.max(0);
        let (plain, old_expiry) = bt::rotate(db, owner, grace)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "owner not found"))?;
        tracing::info!(
            "rotated bootstrap token owner={} grace_period_sec={}",
            owner,
            grace
        );
        Ok(RotateBootstrapTokenResponse {
            owner: owner.to_string(),
            token: plain,
            old_token_expires_at: old_expiry,
        })
    }

    /// 启用 / 禁用。
    pub async fn set_status(
        db: &crate::db::DbPool,
        owner: &str,
        status: &str,
    ) -> Result<BootstrapTokenDto, (StatusCode, &'static str)> {
        validate_owner(owner)?;
        if status != "active" && status != "disabled" {
            return Err((StatusCode::BAD_REQUEST, "status must be active|disabled"));
        }
        let ok = bt::set_status(db, owner, status).await.map_err(internal)?;
        if !ok {
            return Err((StatusCode::NOT_FOUND, "owner not found"));
        }
        let row = bt::find_by_owner(db, owner)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "owner not found"))?;
        tracing::info!("set bootstrap token owner={} status={}", owner, status);
        Ok(BootstrapTokenDto::from(row))
    }

    /// 删除（含宽限期遗留行）。
    pub async fn delete(
        db: &crate::db::DbPool,
        owner: &str,
    ) -> Result<StatusCode, (StatusCode, &'static str)> {
        validate_owner(owner)?;
        let ok = bt::delete(db, owner).await.map_err(internal)?;
        if !ok {
            return Err((StatusCode::NOT_FOUND, "owner not found"));
        }
        tracing::info!("deleted bootstrap token owner={}", owner);
        Ok(StatusCode::NO_CONTENT)
    }
}

// ── service-jwt 路由（CI / 运维脚本）─────────────────────────────────────────

/// GET /admin/bootstrap-tokens — 列出全部（不含明文）。
#[utoipa::path(
    get,
    path = "/admin/bootstrap-tokens",
    summary = "List bootstrap tokens (no plaintext secrets)",
    responses((status = 200, body = ListBootstrapTokensResponse)),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_list(
    State(state): State<TokenServerState>,
) -> Result<Json<ListBootstrapTokensResponse>, (StatusCode, &'static str)> {
    Ok(Json(ops::list(&state.db).await?))
}

/// GET /admin/bootstrap-tokens/{owner} — 查单条（不含明文）。
#[utoipa::path(
    get,
    path = "/admin/bootstrap-tokens/{owner}",
    summary = "Get one bootstrap token metadata (no plaintext secret)",
    params(("owner" = String, Path,)),
    responses(
        (status = 200, body = BootstrapTokenDto),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_get(
    State(state): State<TokenServerState>,
    Path(owner): Path<String>,
) -> Result<Json<BootstrapTokenDto>, (StatusCode, &'static str)> {
    Ok(Json(ops::get(&state.db, &owner).await?))
}

/// POST /admin/bootstrap-tokens — 新建（返回明文一次）。
#[utoipa::path(
    post,
    path = "/admin/bootstrap-tokens",
    summary = "Create a bootstrap token (returns plaintext once)",
    request_body = CreateBootstrapTokenRequest,
    responses(
        (status = 200, body = BootstrapTokenCreated),
        (status = 400, description = "bad request"),
        (status = 409, description = "owner already exists"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_create(
    State(state): State<TokenServerState>,
    Json(req): Json<CreateBootstrapTokenRequest>,
) -> Result<Json<BootstrapTokenCreated>, (StatusCode, &'static str)> {
    Ok(Json(ops::create(&state.db, &req).await?))
}

/// POST /admin/bootstrap-tokens/{owner}/rotate — 换新 token（返回明文一次）。
#[utoipa::path(
    post,
    path = "/admin/bootstrap-tokens/{owner}/rotate",
    summary = "Rotate a bootstrap token (returns new plaintext once)",
    params(("owner" = String, Path,), RotateQuery),
    responses(
        (status = 200, body = RotateBootstrapTokenResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_rotate(
    State(state): State<TokenServerState>,
    Path(owner): Path<String>,
    Query(q): Query<RotateQuery>,
) -> Result<Json<RotateBootstrapTokenResponse>, (StatusCode, &'static str)> {
    Ok(Json(
        ops::rotate(&state.db, &owner, q.grace_period_sec.unwrap_or(0)).await?,
    ))
}

/// PUT /admin/bootstrap-tokens/{owner}/status — 启用 / 禁用。
#[utoipa::path(
    put,
    path = "/admin/bootstrap-tokens/{owner}/status",
    summary = "Enable or disable a bootstrap token",
    params(("owner" = String, Path,)),
    request_body = SetStatusRequest,
    responses(
        (status = 200, body = BootstrapTokenDto),
        (status = 400, description = "bad request"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_set_status(
    State(state): State<TokenServerState>,
    Path(owner): Path<String>,
    Json(req): Json<SetStatusRequest>,
) -> Result<Json<BootstrapTokenDto>, (StatusCode, &'static str)> {
    Ok(Json(ops::set_status(&state.db, &owner, &req.status).await?))
}

/// DELETE /admin/bootstrap-tokens/{owner} — 删除（含宽限期遗留行）。
#[utoipa::path(
    delete,
    path = "/admin/bootstrap-tokens/{owner}",
    summary = "Delete a bootstrap token",
    params(("owner" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_delete(
    State(state): State<TokenServerState>,
    Path(owner): Path<String>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    ops::delete(&state.db, &owner).await
}

/// `/admin/bootstrap-tokens*` 路由（带 service-jwt 鉴权，aud=token-service）。
///
/// 这是**凭据发放面**——能 rotate 就等于能冒充任意服务，故必须鉴权，
/// 且与 `/admin/oauth-clients*` 用同一套 middleware。
pub fn routes(state: crate::TokenServerState) -> axum::Router {
    use axum::routing::{get, post};

    axum::Router::new()
        .route(
            "/admin/bootstrap-tokens",
            get(handle_list).post(handle_create),
        )
        .route(
            "/admin/bootstrap-tokens/{owner}",
            get(handle_get).delete(handle_delete),
        )
        .route(
            "/admin/bootstrap-tokens/{owner}/rotate",
            post(handle_rotate),
        )
        .route(
            "/admin/bootstrap-tokens/{owner}/status",
            axum::routing::put(handle_set_status),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<crate::TokenServerState>(
                "token-service",
            ),
        ))
        .with_state(state)
}
