use crate::dto::{ListResponse, PageQuery, PatchExtendsReq, UpdateMeReq};
use crate::error::AppError;
use crate::service::user_svc;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Extension, Json,
};
use jwt_ctx::Principal;
use serde_json::json;
use crate::db::DbPool;

pub async fn get_current_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
) -> Result<impl IntoResponse, AppError> {
    let uid: u64 = principal
        .user_id
        .parse()
        .map_err(|_| AppError::Unauthorized)?;
    let dto = user_svc::fetch_user_with_roles(&pool, uid).await?;
    Ok(Json(dto))
}

pub async fn update_current_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<UpdateMeReq>,
) -> Result<impl IntoResponse, AppError> {
    let uid: u64 = principal
        .user_id
        .parse()
        .map_err(|_| AppError::Unauthorized)?;
    let profile = json!(req.profile);
    if let DbPool::Mysql(p) = &pool {
        sqlx::query(
            "UPDATE users SET email = ?, phone = ?, nickname = ?, profile_json = ?, \
             updated_at = NOW() WHERE id = ?",
        )
        .bind(req.email)
        .bind(req.phone)
        .bind(req.nickname)
        .bind(serde_json::to_string(&profile).unwrap())
        .bind(uid)
        .execute(p)
        .await?;
    }
    let dto = user_svc::fetch_user_with_roles(&pool, uid).await?;
    Ok(Json(dto))
}

pub async fn list_users(
    State(pool): State<DbPool>,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    let items = user_svc::list_users(&pool, q.page, q.size).await?;
    Ok(Json(ListResponse {
        items,
        page: q.page,
        size: q.size,
    }))
}

pub async fn get_user(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let dto = user_svc::fetch_user_with_roles(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn patch_user_extends(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<PatchExtendsReq>,
) -> Result<impl IntoResponse, AppError> {
    user_svc::update_user_extends(&pool, id, &json!(req.extends)).await?;
    Ok(Json(json!({ "ok": true })))
}
