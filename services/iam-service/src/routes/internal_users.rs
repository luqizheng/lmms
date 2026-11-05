use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use digua_common_db::db_fetch_optional;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    dto::{
        BatchGetUsersRequest, BatchGetUsersResponse, QueryUsersByOrgAndRoleRequest,
        UserClaimsResponse, UserIdListResponse, UserListQuery,
    },
    service::UserService,
    state::AppState,
};
use digua_jwt_ctx::{Principal, TenantScope};

#[derive(Debug, Deserialize, IntoParams)]
pub struct TenantIdQuery {
    pub tenant_id: i64,
}

/// Token-service fetches a user's claims (roles/permissions) to stamp onto
/// issued tokens. 2026-11-05 review (P0-3): this endpoint used to live in
/// `users.rs` with only a `TenantScope` extractor — any valid user-jwt could
/// read arbitrary same-tenant users' PII and permission matrix, and the
/// gateway's dot-segment normalization gap made it reachable as
/// `/v1/iam/users/../../internal/users/{id}/claims`. It now lives in this
/// router, which is wrapped by `verify_aud_principal_middleware("iam-service")`
/// (see routes/mod.rs) so only service-jwts with `aud=iam-service` may call it.
///
/// `tenant_id` is optional: when absent (token-service's client_credentials
/// path), the user is resolved globally by id and their real tenant drives the
/// roles/permissions queries.
#[utoipa::path(
    get,
    path = "/internal/users/{user_id}/claims",
    params(("user_id" = String, Path,), TenantIdQuery),
    responses(
        (status = 200, body = UserClaimsResponse),
        (status = 401, description = "service-jwt required (aud=iam-service)"),
        (status = 404, description = "user not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal"
)]
// check-tenant-scope: delegated (caller is the token-service service-jwt;
// tenant resolved server-side from the user row)
pub async fn get_user_claims(
    State(state): State<AppState>,
    _principal: Principal,
    Path(user_id): Path<String>,
    Query(tq): Query<Option<TenantIdQuery>>,
) -> Result<Json<UserClaimsResponse>, (axum::http::StatusCode, &'static str)> {
    let scope = match tq {
        Some(q) if q.tenant_id > 0 => TenantScope::Single(q.tenant_id),
        _ => TenantScope::CrossTenant,
    };
    let service = UserService::new(state);
    let response = service
        .get_claims(&user_id, &scope)
        .await
        .map_err(|e| match e {
            crate::error::IamError::UserNotFound => {
                (axum::http::StatusCode::NOT_FOUND, "user not found")
            }
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ),
        })?;
    Ok(Json(response))
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
    // 池级单语句 → db_fetch_optional! 宏（`?` 占位符由宏按方言渲染；tid 按 i64 绑定）。
    let org_row: Option<(String,)> = db_fetch_optional!(
        db,
        (String,),
        "SELECT id FROM iam_org_units WHERE code = ? AND tenant_id = ? AND deleted_at IS NULL",
        &req.org_path,
        tid
    )
    .map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "db error".to_string(),
        )
    })?;
    let (org_unit_id,) = org_row.ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("org unit {} not found", req.org_path),
    ))?;

    // Resolve role_id from role_name (code field in iam_roles)
    let role_row: Option<(String,)> = db_fetch_optional!(
        db,
        (String,),
        "SELECT id FROM iam_roles WHERE code = ? AND tenant_id = ? AND deleted_at IS NULL",
        &req.role_name,
        tid
    )
    .map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "db error".to_string(),
        )
    })?;
    let (role_id,) = role_row.ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("role {} not found", req.role_name),
    ))?;

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
        .route("/internal/users/{user_id}/claims", get(get_user_claims))
        .with_state(state)
}
