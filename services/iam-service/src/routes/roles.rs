use axum::{Router, routing::{get, put}, extract::{Json, Path, Query, State}};
use jwt_ctx::{Principal, TenantScope};
use crate::{
    state::AppState,
    dto::{
        CreateRoleRequest, UpdateRoleRequest, RoleQuery, RoleResponse, RoleListResponse,
        AssignPermissionsRequest, AssignUserRolesRequest, AssignUserRolesResponse,
    },
    service::RoleService,
    error::IamError,
};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/roles", get(list_roles).post(create_role))
        .route("/iam/roles/{role_id}", get(get_role).put(update_role).delete(delete_role))
        .route("/iam/roles/{role_id}/permissions", put(assign_permissions))
        .route("/iam/users/{user_id}/roles", put(assign_user_roles))
        .with_state(state)
}

// check-tenant-scope: delegated
async fn list_roles(
    State(state): State<AppState>,
    scope: TenantScope,
    Query(query): Query<RoleQuery>,
) -> Result<Json<RoleListResponse>, (axum::http::StatusCode, &'static str)> {
    let service = RoleService::new(state);
    let response = service.list(query, &scope).await.map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(response))
}

// check-tenant-scope: delegated
async fn create_role(
    State(state): State<AppState>,
    scope: TenantScope,
    Json(req): Json<CreateRoleRequest>,
) -> Result<Json<RoleResponse>, (axum::http::StatusCode, &'static str)> {
    let service = RoleService::new(state);
    let response = service.create(req, &scope).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateRole => (axum::http::StatusCode::CONFLICT, "duplicate role"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

// check-tenant-scope: delegated
async fn get_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    scope: TenantScope,
) -> Result<Json<RoleResponse>, (axum::http::StatusCode, &'static str)> {
    let service = RoleService::new(state);
    let response = service.get(&role_id, &scope).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => (axum::http::StatusCode::NOT_FOUND, "role not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

async fn update_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Principal { user_id: p, .. }: Principal,
    scope: TenantScope,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, (axum::http::StatusCode, &'static str)> {
    let service = RoleService::new(state);
    let response = service.update(&role_id, req, Some(p.as_str()), &scope).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => (axum::http::StatusCode::NOT_FOUND, "role not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

async fn delete_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Principal { user_id: p, .. }: Principal,
    scope: TenantScope,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = RoleService::new(state);
    service.delete(&role_id, Some(p.as_str()), &scope).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => (axum::http::StatusCode::NOT_FOUND, "role not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn assign_permissions(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Principal { user_id: p, .. }: Principal,
    scope: TenantScope,
    Json(req): Json<AssignPermissionsRequest>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = RoleService::new(state);
    service.assign_permissions(&role_id, req.permission_ids, Some(p.as_str()), &scope).await.map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/iam/users/{user_id}/roles",
    params(("user_id" = String, Path,)),
    request_body = AssignUserRolesRequest,
    responses(
        (status = 200, body = AssignUserRolesResponse),
        (status = 400, description = "invalid role ids"),
        (status = 404, description = "user not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
async fn assign_user_roles(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Principal { user_id: p, .. }: Principal,
    scope: TenantScope,
    Json(req): Json<AssignUserRolesRequest>,
) -> Result<Json<AssignUserRolesResponse>, (axum::http::StatusCode, String)> {
    let svc = RoleService::new(state);
    let role_ids = svc.assign_user_roles(&user_id, req.role_ids, Some(p.as_str()), &scope).await.map_err(|e| match e {
        IamError::UserNotFound => (axum::http::StatusCode::NOT_FOUND, "user not found".to_string()),
        IamError::InvalidRoleIds(v) => (
            axum::http::StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": "invalid role_ids", "detail": v }).to_string(),
        ),
        _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string()),
    })?;
    Ok(Json(AssignUserRolesResponse { user_id, role_ids }))
}