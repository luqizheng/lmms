use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use sqlx::Row;
use utoipa::IntoParams;

use crate::{
    dto::{
        BatchGetUsersRequest, BatchGetUsersResponse, QueryUsersByOrgAndRoleRequest,
        UserIdListResponse, UserListQuery,
    },
    service::UserService,
    state::AppState,
};
use digua_jwt_ctx::{Principal, TenantScope};

#[derive(Debug, Deserialize, IntoParams)]
pub struct TenantIdQuery {
    pub tenant_id: i64,
}

#[utoipa::path(
    get,
    path = "/iam/internal/users/by-role/{role_id}",
    params(
        ("role_id" = String, Path,),
        UserListQuery,
        TenantIdQuery,
    ),
    responses(
        (status = 200, body = UserIdListResponse),
        (status = 400, description = "missing or invalid tenant_id"),
        (status = 401, description = "mesh intention denied"),
    ),
    tag = "internal",
)]
pub async fn list_users_by_role(
    State(state): State<AppState>,
    _principal: Principal,
    Path(role_id): Path<String>,
    Query(q): Query<UserListQuery>,
    Query(tq): Query<TenantIdQuery>,
) -> Result<Json<UserIdListResponse>, (axum::http::StatusCode, String)> {
    let svc = UserService::new(state);
    let scope = TenantScope::Single(tq.tenant_id);
    let (items, total, truncated) = svc
        .list_by_role(&role_id, q.page, q.size, &scope)
        .await
        .map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            )
        })?;
    Ok(Json(UserIdListResponse {
        items,
        page: q.page,
        size: q.size.min(1000),
        total,
        truncated,
    }))
}

#[utoipa::path(
    get,
    path = "/iam/internal/users/by-dept/{dept_id}",
    params(
        ("dept_id" = String, Path,),
        UserListQuery,
        TenantIdQuery,
    ),
    responses(
        (status = 200, body = UserIdListResponse),
        (status = 400, description = "missing or invalid tenant_id"),
        (status = 401, description = "mesh intention denied"),
    ),
    tag = "internal",
)]
pub async fn list_users_by_dept(
    State(state): State<AppState>,
    _principal: Principal,
    Path(dept_id): Path<String>,
    Query(q): Query<UserListQuery>,
    Query(tq): Query<TenantIdQuery>,
) -> Result<Json<UserIdListResponse>, (axum::http::StatusCode, String)> {
    let svc = UserService::new(state);
    let scope = TenantScope::Single(tq.tenant_id);
    let (items, total, truncated) = svc
        .list_by_dept(&dept_id, q.page, q.size, &scope)
        .await
        .map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            )
        })?;
    Ok(Json(UserIdListResponse {
        items,
        page: q.page,
        size: q.size.min(1000),
        total,
        truncated,
    }))
}

#[utoipa::path(
    post,
    path = "/iam/internal/users/by-ids",
    params(TenantIdQuery),
    request_body = BatchGetUsersRequest,
    responses(
        (status = 200, body = BatchGetUsersResponse),
        (status = 400, description = "missing or invalid tenant_id"),
        (status = 401, description = "mesh intention denied"),
    ),
    tag = "internal",
)]
pub async fn batch_get_users_by_ids(
    State(state): State<AppState>,
    _principal: Principal,
    Query(tq): Query<TenantIdQuery>,
    Json(req): Json<BatchGetUsersRequest>,
) -> Result<Json<BatchGetUsersResponse>, (axum::http::StatusCode, String)> {
    let svc = UserService::new(state);
    let scope = TenantScope::Single(tq.tenant_id);
    let users = svc
        .batch_get_by_ids(&req.user_ids, &scope)
        .await
        .map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            )
        })?;
    Ok(Json(BatchGetUsersResponse { users }))
}

#[utoipa::path(
    post,
    path = "/iam/internal/users/by-org-and-role",
    params(TenantIdQuery),
    request_body = QueryUsersByOrgAndRoleRequest,
    responses(
        (status = 200, body = BatchGetUsersResponse),
        (status = 400, description = "missing or invalid tenant_id"),
        (status = 401, description = "mesh intention denied"),
    ),
    tag = "internal",
)]
pub async fn list_users_by_org_and_role(
    State(state): State<AppState>,
    _principal: Principal,
    Query(tq): Query<TenantIdQuery>,
    Json(req): Json<QueryUsersByOrgAndRoleRequest>,
) -> Result<Json<BatchGetUsersResponse>, (axum::http::StatusCode, String)> {
    let tid = tq.tenant_id;
    let db = &state.db;

    // Resolve org_unit_id from org_path (code field in iam_org_units)
    let org_row = sqlx::query(
        "SELECT id FROM iam_org_units WHERE code = ? AND tenant_id = ? AND deleted_at IS NULL",
    )
    .bind(&req.org_path)
    .bind(tid)
    .fetch_optional(db)
    .await
    .map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "db error".to_string(),
        )
    })?
    .ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("org unit {} not found", req.org_path),
    ))?;
    let org_unit_id: String = org_row.get("id");

    // Resolve role_id from role_name (code field in iam_roles)
    let role_row = sqlx::query(
        "SELECT id FROM iam_roles WHERE code = ? AND tenant_id = ? AND deleted_at IS NULL",
    )
    .bind(&req.role_name)
    .bind(tid)
    .fetch_optional(db)
    .await
    .map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "db error".to_string(),
        )
    })?
    .ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("role {} not found", req.role_name),
    ))?;
    let role_id: String = role_row.get("id");

    let svc = UserService::new(state);
    let scope = TenantScope::Single(tid);
    let users = svc
        .list_by_org_and_role(&org_unit_id, &role_id, &scope)
        .await
        .map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            )
        })?;
    Ok(Json(BatchGetUsersResponse { users }))
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/iam/internal/users/by-role/{role_id}",
            get(list_users_by_role),
        )
        .route(
            "/iam/internal/users/by-dept/{dept_id}",
            get(list_users_by_dept),
        )
        .route("/iam/internal/users/by-ids", post(batch_get_users_by_ids))
        .route(
            "/iam/internal/users/by-org-and-role",
            post(list_users_by_org_and_role),
        )
        .with_state(state)
}
