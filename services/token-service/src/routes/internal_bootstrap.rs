//! LMMS Bootstrap provisioning 端点（WP-BS-01 / WP-01D）。
//!
//! IAM 平台初始化（Bootstrap）流程对 token-service 的内部调用面：
//!
//! | 路由 | 鉴权 | 语义 |
//! |---|---|---|
//! | `GET /internal/bootstrap/readiness` | service-jwt（`aud=token-service`） | 必需服务身份 + 必需系统 Grant 状态（无凭据材料） |
//! | `POST /internal/bootstrap/ensure-grants` | 同上 | 幂等补插缺失的 code-owned 系统 Grant（`system_managed = TRUE`） |
//!
//! 设计基线：docs/superpowers/specs/2026-11-28-bootstrap-concurrency-control-design.md
//! Q12——内部服务身份与授权关系**完全由 LMMS（本服务）定义**：必需集合
//! 直接取自 `service_jwt_grant::SYSTEM_GRANTS`（单一真相源），请求方
//! （iam-service）不传任何拓扑；响应不含任何凭据材料。
//!
//! 幂等键 = (owner, audience) 主键：重试 / 恢复不产生重复 Grant；已存在
//! 但被人工禁用的行**不覆盖**，作为 conflict 上报（fail-closed，与
//! `ensure_system_grants` "只补缺失行、不复活显式撤销授权" 语义一致）。

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::{bootstrap_token as bt, service_jwt_grant as grant};
use crate::TokenServerState;

/// 必需服务身份状态（LMMS 内部清单 = SYSTEM_GRANTS owner 并集）。
#[derive(Debug, Serialize, ToSchema)]
pub struct InternalIdentityState {
    pub owner: String,
    pub exists: bool,
    pub status: Option<String>,
}

/// 必需系统 Grant 状态（无凭据材料）。
#[derive(Debug, Serialize, ToSchema)]
pub struct InternalGrantState {
    pub owner: String,
    pub audience: String,
    pub exists: bool,
    pub status: Option<String>,
    pub system_managed: Option<bool>,
}

/// GET /internal/bootstrap/readiness 响应。
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadinessResponse {
    pub identities: Vec<InternalIdentityState>,
    pub grants: Vec<InternalGrantState>,
}

/// POST /internal/bootstrap/ensure-grants 响应。
#[derive(Debug, Serialize, ToSchema)]
pub struct EnsureGrantsResponse {
    /// ensure 后处于 active 的系统 Grant（含本次补插与既有）。
    pub ensured: Vec<InternalGrantState>,
    /// 已存在但被禁用的系统 Grant（不覆盖，fail-closed 上报）。
    pub conflicts: Vec<InternalGrantState>,
}

/// ensure-grants 请求体（预留；当前无参数——必需集合 code-owned）。
#[derive(Debug, Deserialize, ToSchema, Default)]
pub struct EnsureGrantsRequest {}

fn grant_state(row: Option<grant::ServiceJwtGrantRow>) -> InternalGrantState {
    match row {
        Some(r) => InternalGrantState {
            owner: r.owner,
            audience: r.audience,
            exists: true,
            status: Some(r.status),
            system_managed: Some(r.system_managed),
        },
        None => InternalGrantState {
            owner: String::new(),
            audience: String::new(),
            exists: false,
            status: None,
            system_managed: None,
        },
    }
}

async fn identity_state(
    pool: &crate::db::DbPool,
    owner: &str,
) -> Result<InternalIdentityState, (StatusCode, String)> {
    let row = bt::find_by_owner(pool, owner)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(match row {
        Some(r) => InternalIdentityState {
            owner: r.owner,
            exists: true,
            status: Some(r.status),
        },
        None => InternalIdentityState {
            owner: owner.to_string(),
            exists: false,
            status: None,
        },
    })
}

#[utoipa::path(
    get,
    path = "/internal/bootstrap/readiness",
    responses(
        (status = 200, body = ReadinessResponse),
        (status = 500, description = "database error"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal-bootstrap"
)]
pub async fn handle_readiness(
    State(state): State<TokenServerState>,
) -> Result<Json<ReadinessResponse>, (StatusCode, String)> {
    let pool = &state.db;

    // 必需身份：SYSTEM_GRANTS 的 owner 并集（LMMS 内部定义，请求不传）。
    let mut owners: Vec<&str> = grant::SYSTEM_GRANTS.iter().map(|(o, _)| *o).collect();
    owners.sort_unstable();
    owners.dedup();

    let mut identities = Vec::with_capacity(owners.len());
    for owner in owners {
        identities.push(identity_state(pool, owner).await?);
    }

    let mut grants = Vec::with_capacity(grant::SYSTEM_GRANTS.len());
    for (owner, audience) in grant::SYSTEM_GRANTS {
        let row = grant::find_by_owner_and_audience(pool, owner, audience)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let mut g = grant_state(row);
        g.owner = (*owner).to_string();
        g.audience = (*audience).to_string();
        grants.push(g);
    }

    Ok(Json(ReadinessResponse { identities, grants }))
}

#[utoipa::path(
    post,
    path = "/internal/bootstrap/ensure-grants",
    request_body = EnsureGrantsRequest,
    responses(
        (status = 200, body = EnsureGrantsResponse),
        (status = 500, description = "database error"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal-bootstrap"
)]
pub async fn handle_ensure_grants(
    State(state): State<TokenServerState>,
    Json(_req): Json<EnsureGrantsRequest>,
) -> Result<Json<EnsureGrantsResponse>, (StatusCode, String)> {
    let pool = &state.db;
    let mut ensured = Vec::with_capacity(grant::SYSTEM_GRANTS.len());
    let mut conflicts = Vec::new();

    for (owner, audience) in grant::SYSTEM_GRANTS {
        let existing = grant::find_by_owner_and_audience(pool, owner, audience)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        match existing {
            Some(row) if row.status == "active" => {
                let mut g = grant_state(Some(row));
                g.owner = (*owner).to_string();
                g.audience = (*audience).to_string();
                ensured.push(g);
            }
            Some(row) => {
                // 已存在但非 active：不覆盖（不复活被显式撤销的授权）。
                let mut g = grant_state(Some(row));
                g.owner = (*owner).to_string();
                g.audience = (*audience).to_string();
                conflicts.push(g);
            }
            None => {
                // 幂等补插（主键唯一约束兜底并发竞态，system_managed=TRUE）。
                grant::provision_system_grant(pool, owner, audience)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                let row = grant::find_by_owner_and_audience(pool, owner, audience)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                let mut g = grant_state(row);
                g.owner = (*owner).to_string();
                g.audience = (*audience).to_string();
                ensured.push(g);
            }
        }
    }

    Ok(Json(EnsureGrantsResponse { ensured, conflicts }))
}

pub fn routes(state: TokenServerState) -> Router {
    Router::new()
        .route(
            "/internal/bootstrap/readiness",
            get(handle_readiness),
        )
        .route(
            "/internal/bootstrap/ensure-grants",
            axum::routing::post(handle_ensure_grants),
        )
        .with_state(state)
}
