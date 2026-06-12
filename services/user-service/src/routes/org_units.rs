use crate::db::DbPool;
use crate::dto::{CreateOrgUnitReq, UpdateOrgUnitReq};
use crate::error::AppError;
use crate::service::org_svc;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct OrgUnitListQuery {
    pub parent_id: Option<u64>,
}

pub async fn list(
    State(pool): State<DbPool>,
    Query(q): Query<OrgUnitListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let items = org_svc::list_org_units(&pool, q.parent_id).await?;
    Ok(Json(items))
}

pub async fn create(
    State(pool): State<DbPool>,
    Json(req): Json<CreateOrgUnitReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = org_svc::create_org_unit(&pool, &req.code, &req.name, req.parent_id, req.sort).await?;
    Ok(Json(dto))
}

pub async fn get(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let dto = org_svc::get_org_unit(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn update(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<UpdateOrgUnitReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = org_svc::update_org_unit(&pool, id, &req.name, req.parent_id, req.sort, req.status)
        .await?;
    Ok(Json(dto))
}

pub async fn delete(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    org_svc::delete_org_unit(&pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn tree(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let items = org_svc::get_org_tree(&pool, id).await?;
    Ok(Json(items))
}

pub fn router() -> axum::Router<DbPool> {
    axum::Router::new()
        .route("/org-units", axum::routing::get(list).post(create))
        .route("/org-units/{id}", axum::routing::get(get).put(update).delete(delete))
        .route("/org-units/{id}/tree", axum::routing::get(tree))
}
