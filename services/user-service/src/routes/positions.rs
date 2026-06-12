use crate::db::DbPool;
use crate::dto::{AssignPositionRolesReq, CreatePositionReq, UpdatePositionReq};
use crate::error::AppError;
use crate::service::position_svc;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PositionListQuery {
    pub org_unit_id: Option<u64>,
}

pub async fn list(
    State(pool): State<DbPool>,
    Query(q): Query<PositionListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let items = position_svc::list_positions(&pool, q.org_unit_id).await?;
    Ok(Json(items))
}

pub async fn create(
    State(pool): State<DbPool>,
    Json(req): Json<CreatePositionReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto =
        position_svc::create_position(&pool, &req.code, &req.name, req.org_unit_id, req.level)
            .await?;
    Ok(Json(dto))
}

pub async fn get(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let dto = position_svc::get_position(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn update(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<UpdatePositionReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = position_svc::update_position(
        &pool,
        id,
        &req.name,
        req.org_unit_id,
        req.level,
        req.status,
    )
    .await?;
    Ok(Json(dto))
}

pub async fn delete(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    position_svc::delete_position(&pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn assign_roles(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignPositionRolesReq>,
) -> Result<impl IntoResponse, AppError> {
    position_svc::assign_position_roles(&pool, id, &req.role_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub fn router() -> axum::Router<DbPool> {
    axum::Router::new()
        .route("/positions", axum::routing::get(list).post(create))
        .route(
            "/positions/:id",
            axum::routing::get(get).put(update).delete(delete),
        )
        .route("/positions/{id}/roles", axum::routing::put(assign_roles))
}
