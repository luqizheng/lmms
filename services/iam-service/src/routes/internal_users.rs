use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    dto::{UserIdListResponse, UserListQuery},
    service::UserService,
    state::AppState,
};

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
    Path(role_id): Path<String>,
    Query(q): Query<UserListQuery>,
    Query(tq): Query<TenantIdQuery>,
) -> Result<Json<UserIdListResponse>, (axum::http::StatusCode, String)> {
    let svc = UserService::new(state);
    let (items, total, truncated) = svc
        .list_by_role(&role_id, q.page, q.size, tq.tenant_id)
        .await
        .map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string()))?;
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
    Path(dept_id): Path<String>,
    Query(q): Query<UserListQuery>,
    Query(tq): Query<TenantIdQuery>,
) -> Result<Json<UserIdListResponse>, (axum::http::StatusCode, String)> {
    let svc = UserService::new(state);
    let (items, total, truncated) = svc
        .list_by_dept(&dept_id, q.page, q.size, tq.tenant_id)
        .await
        .map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string()))?;
    Ok(Json(UserIdListResponse {
        items,
        page: q.page,
        size: q.size.min(1000),
        total,
        truncated,
    }))
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/internal/users/by-role/{role_id}", get(list_users_by_role))
        .route("/iam/internal/users/by-dept/{dept_id}", get(list_users_by_dept))
        .with_state(state)
}
