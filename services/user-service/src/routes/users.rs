use crate::db::DbPool;
use crate::dto::{
    AdminResetPasswordReq, ChangePasswordReq, CreateUserReq, ListResponse, PageQuery,
    UpdateUserReq, UpdateMeReq,
};
use crate::error::AppError;
use crate::service::user_svc;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use jwt_ctx::Principal;
use serde_json::json;

fn require_admin(p: &Principal) -> Result<(), AppError> {
    if p.has_role("admin") {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn parse_uid(p: &Principal) -> Result<u64, AppError> {
    p.user_id.parse().map_err(|_| AppError::Unauthorized)
}

pub async fn get_current_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
) -> Result<impl IntoResponse, AppError> {
    let uid = parse_uid(&principal)?;
    let dto = user_svc::fetch_user_with_roles(&pool, uid).await?;
    Ok(Json(dto))
}

pub async fn update_current_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<UpdateMeReq>,
) -> Result<impl IntoResponse, AppError> {
    let uid = parse_uid(&principal)?;
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

pub async fn change_my_password(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<ChangePasswordReq>,
) -> Result<impl IntoResponse, AppError> {
    let uid = parse_uid(&principal)?;
    user_svc::change_password(&pool, uid, &req).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_users(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let items = user_svc::list_users(&pool, q.page, q.size).await?;
    Ok(Json(ListResponse {
        items,
        page: q.page,
        size: q.size,
    }))
}

pub async fn get_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let dto = user_svc::fetch_user_with_roles(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn create_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<CreateUserReq>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let uid = user_svc::create_user(&pool, &req).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({ "id": uid, "username": req.username })),
    ))
}

pub async fn update_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
    Json(req): Json<UpdateUserReq>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    user_svc::update_user_basic(&pool, id, &req).await?;
    let dto = user_svc::fetch_user_with_roles(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn delete_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let me = parse_uid(&principal)?;
    if me == id {
        return Err(AppError::Validation("cannot delete yourself".into()));
    }
    user_svc::soft_delete_user(&pool, id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn enable_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let me = parse_uid(&principal)?;
    if me == id {
        return Err(AppError::Validation("cannot disable yourself".into()));
    }
    user_svc::set_user_status(&pool, id, 1).await?;
    Ok(Json(json!({ "ok": true, "status": 1 })))
}

pub async fn disable_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let me = parse_uid(&principal)?;
    if me == id {
        return Err(AppError::Validation("cannot disable yourself".into()));
    }
    user_svc::set_user_status(&pool, id, 0).await?;
    Ok(Json(json!({ "ok": true, "status": 0 })))
}

pub async fn admin_reset_password(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
    Json(req): Json<AdminResetPasswordReq>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    user_svc::reset_password(&pool, id, &req.new_password).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_user_login_logs(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    let me = parse_uid(&principal)?;
    if !principal.has_role("admin") && me != id {
        return Err(AppError::Forbidden);
    }
    let items = user_svc::list_login_logs(&pool, id, q.page, q.size).await?;
    Ok(Json(ListResponse {
        items,
        page: q.page,
        size: q.size,
    }))
}

pub fn router() -> Router<DbPool> {
    Router::new()
        .route("/users", post(create_user).get(list_users))
        .route("/users/me", get(get_current_user).patch(update_current_user))
        .route("/users/me/password", post(change_my_password))
        .route("/users/{id}", get(get_user).patch(update_user).delete(delete_user))
        .route("/users/{id}/enable", post(enable_user))
        .route("/users/{id}/disable", post(disable_user))
        .route("/users/{id}/reset-password", post(admin_reset_password))
        .route("/users/{id}/login-logs", get(list_user_login_logs))
}
