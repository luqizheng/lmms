use axum::{Router, routing::get, extract::{Json, Path, Query, State}};
use jwt_ctx::Principal;
use crate::{state::AppState, dto::{CreatePermissionRequest, UpdatePermissionRequest, PermissionQuery, PermissionResponse, PermissionListResponse}, service::PermissionService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/permissions", get(list_permissions).post(create_permission))
        .route("/iam/permissions/{permission_id}", get(get_permission).put(update_permission).delete(delete_permission))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/iam/permissions",
    params(PermissionQuery),
    responses((status = 200, body = PermissionListResponse)),
    security(("bearer_auth" = [])),
    tag = "permissions"
)]
async fn list_permissions(
    State(state): State<AppState>,
    Query(query): Query<PermissionQuery>,
) -> Result<Json<PermissionListResponse>, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    let response = service.list(query).await.map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/iam/permissions",
    request_body = CreatePermissionRequest,
    responses(
        (status = 200, body = PermissionResponse),
        (status = 409, description = "duplicate permission"),
    ),
    security(("bearer_auth" = [])),
    tag = "permissions"
)]
async fn create_permission(
    State(state): State<AppState>,
    Principal { user_id: p, .. }: Principal,
    Json(req): Json<CreatePermissionRequest>,
) -> Result<Json<PermissionResponse>, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    let response = service.create(req, Some(p.as_str())).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicatePermission => (axum::http::StatusCode::CONFLICT, "duplicate permission"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/iam/permissions/{permission_id}",
    params(("permission_id" = String, Path, description = "Permission ID")),
    responses(
        (status = 200, body = PermissionResponse),
        (status = 404, description = "permission not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "permissions"
)]
async fn get_permission(
    State(state): State<AppState>,
    Path(permission_id): Path<String>,
) -> Result<Json<PermissionResponse>, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    let response = service.get(&permission_id).await.map_err(|e| {
        match e {
            crate::error::IamError::PermissionNotFound => (axum::http::StatusCode::NOT_FOUND, "permission not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/iam/permissions/{permission_id}",
    params(("permission_id" = String, Path, description = "Permission ID")),
    request_body = UpdatePermissionRequest,
    responses(
        (status = 200, body = PermissionResponse),
        (status = 404, description = "permission not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "permissions"
)]
async fn update_permission(
    State(state): State<AppState>,
    Path(permission_id): Path<String>,
    Principal { user_id: p, .. }: Principal,
    Json(req): Json<UpdatePermissionRequest>,
) -> Result<Json<PermissionResponse>, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    let response = service.update(&permission_id, req, Some(p.as_str())).await.map_err(|e| {
        match e {
            crate::error::IamError::PermissionNotFound => (axum::http::StatusCode::NOT_FOUND, "permission not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/iam/permissions/{permission_id}",
    params(("permission_id" = String, Path, description = "Permission ID")),
    responses(
        (status = 204, description = "permission deleted"),
        (status = 404, description = "permission not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "permissions"
)]
async fn delete_permission(
    State(state): State<AppState>,
    Path(permission_id): Path<String>,
    Principal { user_id: p, .. }: Principal,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    service.delete(&permission_id, Some(p.as_str())).await.map_err(|e| {
        match e {
            crate::error::IamError::PermissionNotFound => (axum::http::StatusCode::NOT_FOUND, "permission not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}