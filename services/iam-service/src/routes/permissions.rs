use axum::{Router, routing::get, extract::{Json, Path, Query, State}};
use crate::{state::AppState, dto::{CreatePermissionRequest, UpdatePermissionRequest, PermissionQuery, PermissionResponse, PermissionListResponse}, service::PermissionService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/permissions", get(list_permissions).post(create_permission))
        .route("/iam/permissions/{permission_id}", get(get_permission).put(update_permission).delete(delete_permission))
        .with_state(state)
}

async fn list_permissions(
    State(state): State<AppState>,
    Query(query): Query<PermissionQuery>,
) -> Result<Json<PermissionListResponse>, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    let response = service.list(query).await.map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(response))
}

async fn create_permission(
    State(state): State<AppState>,
    Json(req): Json<CreatePermissionRequest>,
) -> Result<Json<PermissionResponse>, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicatePermission => (axum::http::StatusCode::CONFLICT, "duplicate permission"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

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

async fn update_permission(
    State(state): State<AppState>,
    Path(permission_id): Path<String>,
    Json(req): Json<UpdatePermissionRequest>,
) -> Result<Json<PermissionResponse>, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    let response = service.update(&permission_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::PermissionNotFound => (axum::http::StatusCode::NOT_FOUND, "permission not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

async fn delete_permission(
    State(state): State<AppState>,
    Path(permission_id): Path<String>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = PermissionService::new(state);
    service.delete(&permission_id).await.map_err(|e| {
        match e {
            crate::error::IamError::PermissionNotFound => (axum::http::StatusCode::NOT_FOUND, "permission not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}