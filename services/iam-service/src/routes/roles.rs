use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, Query, State}};
use crate::{state::AppState, dto::{CreateRoleRequest, UpdateRoleRequest, RoleQuery, RoleResponse, RoleListResponse, AssignPermissionsRequest}, service::RoleService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/roles", get(list_roles).post(create_role))
        .route("/iam/roles/:role_id", get(get_role).put(update_role).delete(delete_role))
        .route("/iam/roles/:role_id/permissions", put(assign_permissions))
        .with_state(state)
}

async fn list_roles(
    State(state): State<AppState>,
    Query(query): Query<RoleQuery>,
) -> Result<Json<RoleListResponse>, axum::http::StatusCode> {
    let service = RoleService::new(state);
    let response = service.list(query).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_role(
    State(state): State<AppState>,
    Json(req): Json<CreateRoleRequest>,
) -> Result<Json<RoleResponse>, axum::http::StatusCode> {
    let service = RoleService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateRole => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
) -> Result<Json<RoleResponse>, axum::http::StatusCode> {
    let service = RoleService::new(state);
    let response = service.get(&role_id).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, axum::http::StatusCode> {
    let service = RoleService::new(state);
    let response = service.update(&role_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = RoleService::new(state);
    service.delete(&role_id).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}

async fn assign_permissions(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Json(req): Json<AssignPermissionsRequest>,
) -> Result<(), axum::http::StatusCode> {
    let service = RoleService::new(state);
    service.assign_permissions(&role_id, req.permission_ids).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
