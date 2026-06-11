use crate::db::DbPool;
use crate::dto::AssignUserPositionsReq;
use crate::error::AppError;
use crate::service::user_position_svc::{self, PositionAssignItem};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

pub async fn list(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let items = user_position_svc::list_user_positions(&pool, id).await?;
    Ok(Json(items))
}

pub async fn replace(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignUserPositionsReq>,
) -> Result<impl IntoResponse, AppError> {
    let items: Vec<PositionAssignItem> = req
        .items
        .into_iter()
        .map(|i| PositionAssignItem {
            position_id: i.position_id,
            is_primary: i.is_primary,
        })
        .collect();
    let items = user_position_svc::replace_user_positions(&pool, id, &items).await?;
    Ok(Json(items))
}

pub fn router() -> axum::Router<DbPool> {
    axum::Router::new().route("/users/:id/positions", axum::routing::get(list).put(replace))
}
