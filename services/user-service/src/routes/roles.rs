use crate::db::DbPool;
use crate::dto::{AssignRolePermissionsReq, CreateRoleReq, UpdateRoleReq};
use crate::error::AppError;
use crate::service::role_svc;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

pub async fn list_roles(State(pool): State<DbPool>) -> Result<impl IntoResponse, AppError> {
    let items = role_svc::list_roles(&pool).await?;
    Ok(Json(items))
}

pub async fn create_role(
    State(pool): State<DbPool>,
    Json(req): Json<CreateRoleReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto =
        role_svc::create_role(&pool, &req.code, &req.name, req.description.as_deref()).await?;
    Ok(Json(dto))
}

pub async fn get_role(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let dto = role_svc::get_role(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn update_role(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<UpdateRoleReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = role_svc::update_role(&pool, id, &req.name, req.description.as_deref()).await?;
    Ok(Json(dto))
}

pub async fn delete_role(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    role_svc::delete_role(&pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn assign_role_permissions(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignRolePermissionsReq>,
) -> Result<impl IntoResponse, AppError> {
    role_svc::assign_role_permissions(&pool, id, &req.permission_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
