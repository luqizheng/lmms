use axum::{
    extract::{Path, Query, State},
    middleware,
    routing::get,
    Json, Router,
};
use jwt_ctx::Principal;

use crate::{
    dto::{UserIdListResponse, UserListQuery},
    middleware::service_jwt::require_service_jwt,
    service::UserService,
    state::AppState,
};

#[utoipa::path(
    get,
    path = "/iam/internal/users/by-role/{role_id}",
    params(
        ("role_id" = String, Path,),
        UserListQuery,
    ),
    responses(
        (status = 200, body = UserIdListResponse),
        (status = 401, description = "service token required"),
    ),
    tag = "internal",
)]
pub async fn list_users_by_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Query(q): Query<UserListQuery>,
    principal: Principal,
) -> Result<Json<UserIdListResponse>, (axum::http::StatusCode, String)> {
    let svc = UserService::new(state);
    let (items, total, truncated) = svc
        .list_by_role(&role_id, q.page, q.size, principal.tenant_id)
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
    ),
    responses(
        (status = 200, body = UserIdListResponse),
        (status = 401, description = "service token required"),
    ),
    tag = "internal",
)]
pub async fn list_users_by_dept(
    State(state): State<AppState>,
    Path(dept_id): Path<String>,
    Query(q): Query<UserListQuery>,
    principal: Principal,
) -> Result<Json<UserIdListResponse>, (axum::http::StatusCode, String)> {
    let svc = UserService::new(state);
    let (items, total, truncated) = svc
        .list_by_dept(&dept_id, q.page, q.size, principal.tenant_id)
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
        .route_layer(middleware::from_fn(require_service_jwt))
        .with_state(state)
}