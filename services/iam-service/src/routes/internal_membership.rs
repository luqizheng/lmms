//! GET /iam/internal/membership-bootstrap — full user→{role_ids, dept_id} map for one tenant.
//!
//! Phase 5B-fu3 (spec §3.5): notification-service 启动时一次性拉取本地 user
//! membership 投影表的初始状态。tenant_id 来自 service-jwt 的 `tid` claim
//! （不接 query 参数 — service-jwt 是 caller 身份的真相源，与 Phase 8.3
//! 跨租户检测方向一致；bootstrap 是 caller 取自己的 tenant 投影，不是
//! "查任意 tenant"，区别于 by-role/by-dept 的 query 参数用法）。
//!
//! 行为：
//! - 鉴权：service-jwt aud=iam-service（沿用现有 routes/mod.rs 的 layer）
//! - 单 SQL LEFT JOIN 全量拉取，server-side 拼 HashMap（10K user 级别单查 < 100ms）
//! - dept_id 取用户的 primary org_unit_id（iam_user_org_units.is_primary=TRUE）
//! - role_ids: GROUP_CONCAT(MySQL) / STRING_AGG(PG)；无角色 → 空 Vec
//! - deleted_at IS NULL 的 user 才返回

use axum::{extract::State, Json};
use digua_jwt_ctx::Principal;
use serde::Serialize;
use std::collections::HashMap;

use crate::state::AppState;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MembershipBootstrap {
    pub tenant_id: i64,
    pub mappings: HashMap<String, UserMembership>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UserMembership {
    pub role_ids: Vec<String>,
    pub dept_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/iam/internal/membership-bootstrap",
    responses(
        (status = 200, body = MembershipBootstrap),
        (status = 401, description = "missing or invalid service-jwt"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal"
)]
pub async fn membership_bootstrap(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<Json<MembershipBootstrap>, (axum::http::StatusCode, String)> {
    let tenant_id = principal.tenant_id;

    // 聚合函数方言差异（GROUP_CONCAT vs STRING_AGG）→ 双臂取池；
    // 臂内直接解成元组（MySqlRow/PgRow 的 try_get 无法跨方言共用）。
    let rows: Vec<(String, Option<String>, Option<String>)> = match &state.db {
        #[cfg(feature = "mysql")]
        digua_common_db::DbPool::Mysql(p) => fetch_membership_rows_mysql(p, tenant_id).await?,
        #[cfg(feature = "postgres")]
        digua_common_db::DbPool::Postgres(p) => fetch_membership_rows_pg(p, tenant_id).await?,
        #[allow(unreachable_patterns)]
        _ => {
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "dialect feature not enabled".to_string(),
            ))
        }
    };

    let mut mappings = HashMap::with_capacity(rows.len());
    for (user_id, dept_id, role_ids_str) in rows {
        let role_ids: Vec<String> = role_ids_str
            .map(|s| {
                s.split(',')
                    .filter(|x| !x.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        mappings.insert(user_id, UserMembership { role_ids, dept_id });
    }

    Ok(Json(MembershipBootstrap {
        tenant_id,
        mappings,
    }))
}

/// MySQL 臂：GROUP_CONCAT 聚合 role_id（逗号分隔无空格）；tenant_id 按 i64 绑定。
#[cfg(feature = "mysql")]
async fn fetch_membership_rows_mysql(
    pool: &sqlx::MySqlPool,
    tenant_id: i64,
) -> Result<Vec<(String, Option<String>, Option<String>)>, (axum::http::StatusCode, String)> {
    sqlx::query_as::<sqlx::MySql, (String, Option<String>, Option<String>)>(
        "SELECT u.id, \
                uox.org_unit_id AS dept_id, \
                GROUP_CONCAT(ur.role_id) AS role_ids \
         FROM iam_users u \
         LEFT JOIN iam_user_roles ur ON ur.user_id = u.id \
         LEFT JOIN iam_user_org_units uox ON uox.user_id = u.id AND uox.is_primary = TRUE \
         WHERE u.tenant_id = ? AND u.deleted_at IS NULL \
         GROUP BY u.id, uox.org_unit_id",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_id, "membership-bootstrap query failed");
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal error".to_string(),
        )
    })
}

/// PG 臂：STRING_AGG(ur.role_id::text, ',') 与 GROUP_CONCAT 输出格式一致（逗号分隔无空格）；$1 占位符。
#[cfg(feature = "postgres")]
async fn fetch_membership_rows_pg(
    pool: &sqlx::PgPool,
    tenant_id: i64,
) -> Result<Vec<(String, Option<String>, Option<String>)>, (axum::http::StatusCode, String)> {
    sqlx::query_as::<sqlx::Postgres, (String, Option<String>, Option<String>)>(
        "SELECT u.id, \
                uox.org_unit_id AS dept_id, \
                STRING_AGG(ur.role_id::text, ',') AS role_ids \
         FROM iam_users u \
         LEFT JOIN iam_user_roles ur ON ur.user_id = u.id \
         LEFT JOIN iam_user_org_units uox ON uox.user_id = u.id AND uox.is_primary = TRUE \
         WHERE u.tenant_id = $1 AND u.deleted_at IS NULL \
         GROUP BY u.id, uox.org_unit_id",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, tenant_id, "membership-bootstrap query failed");
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal error".to_string(),
        )
    })
}

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route(
            "/iam/internal/membership-bootstrap",
            axum::routing::get(membership_bootstrap),
        )
        .with_state(state)
}
