use crate::dto::AssignUserRolesReq;
use crate::error::AppError;
use crate::service::user_svc;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use sqlx::MySqlPool;

pub async fn assign_user_roles(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignUserRolesReq>,
) -> Result<impl IntoResponse, AppError> {
    user_svc::assign_user_roles(&pool, id, &req.role_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn remove_user_roles(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignUserRolesReq>,
) -> Result<impl IntoResponse, AppError> {
    user_svc::remove_user_roles(&pool, id, &req.role_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
