use crate::auth::{AdminCheck, Identity};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::redis_cli::RedisCli;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

#[derive(Clone)]
pub struct RestState {
    pub db: Db,
    pub redis: RedisCli,
}

pub fn router(state: RestState) -> Router {
    Router::new()
        .route("/announcements", post(create).get(list_mine))
        .route("/announcements/{id}", get(detail))
        .route("/announcements/{id}/publish", post(publish))
        .route("/announcements/{id}/revoke", post(revoke))
        .route("/announcements/{id}/read", post(read_one))
        .route("/unread-count", get(unread_count))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/notification/announcements",
    request_body = CreateAnnouncement,
    responses(
        (status = 201, description = "创建成功", body = serde_json::Value),
        (status = 403, description = "无权限"),
    ),
    tag = "announcements"
)]
pub async fn create(
    State(s): State<RestState>,
    me: Identity,
    Json(body): Json<CreateAnnouncement>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    if !me.is_admin() {
        return Err(AppError::Forbidden);
    }
    if body.title.is_empty() || body.recipients.is_empty() {
        return Err(AppError::BadRequest("title and recipients required".into()));
    }
    let id = s.db.create_announcement(&body, me.user_id).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

#[utoipa::path(
    post,
    path = "/notification/announcements/{id}/publish",
    params(("id" = u64, Path, description = "公告ID")),
    responses(
        (status = 200, description = "发布成功", body = serde_json::Value),
        (status = 403, description = "无权限"),
    ),
    tag = "announcements"
)]
pub async fn publish(
    State(s): State<RestState>,
    me: Identity,
    Path(id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    if !me.is_admin() {
        return Err(AppError::Forbidden);
    }
    s.db.set_status(id, Status::Published).await?;
    Ok(Json(serde_json::json!({ "id": id, "status": "published" })))
}

#[utoipa::path(
    post,
    path = "/notification/announcements/{id}/revoke",
    params(("id" = u64, Path, description = "公告ID")),
    responses(
        (status = 200, description = "撤销成功", body = serde_json::Value),
        (status = 403, description = "无权限"),
    ),
    tag = "announcements"
)]
pub async fn revoke(
    State(s): State<RestState>,
    me: Identity,
    Path(id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    if !me.is_admin() {
        return Err(AppError::Forbidden);
    }
    s.db.set_status(id, Status::Revoked).await?;
    Ok(Json(serde_json::json!({ "id": id, "status": "revoked" })))
}

#[utoipa::path(
    get,
    path = "/notification/announcements/{id}",
    params(("id" = u64, Path, description = "公告ID")),
    responses(
        (status = 200, description = "获取成功", body = Announcement),
        (status = 404, description = "公告不存在"),
    ),
    tag = "announcements"
)]
pub async fn detail(
    State(s): State<RestState>,
    _me: Identity,
    Path(id): Path<u64>,
) -> AppResult<Json<Announcement>> {
    s.db.get(id).await?.map(Json).ok_or(AppError::NotFound)
}

#[derive(Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
pub struct ListQuery {
    #[param(default)]
    pinned: Option<bool>,
    #[param(default)]
    active: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/notification/announcements",
    params(ListQuery),
    responses(
        (status = 200, description = "获取成功", body = Vec<Announcement>),
    ),
    tag = "announcements"
)]
pub async fn list_mine(
    State(s): State<RestState>,
    me: Identity,
    _q: Query<ListQuery>,
) -> AppResult<Json<Vec<Announcement>>> {
    Ok(Json(s.db.list_for_user(me.user_id).await?))
}

#[utoipa::path(
    post,
    path = "/notification/announcements/{id}/read",
    params(("id" = u64, Path, description = "公告ID")),
    responses(
        (status = 200, description = "标记已读", body = serde_json::Value),
    ),
    tag = "notifications"
)]
pub async fn read_one(
    State(s): State<RestState>,
    me: Identity,
    Path(id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    let affected = s.db.mark_read(id, me.user_id.clone()).await?;
    if affected {
        s.redis.decr_unread_floor0(me.user_id).await.ok();
    }
    Ok(Json(serde_json::json!({ "ok": true, "changed": affected })))
}

#[utoipa::path(
    get,
    path = "/notification/unread-count",
    responses(
        (status = 200, description = "获取未读数", body = serde_json::Value),
    ),
    tag = "notifications"
)]
pub async fn unread_count(
    State(s): State<RestState>,
    me: Identity,
) -> AppResult<Json<serde_json::Value>> {
    let c = s.redis.get_unread(me.user_id).await.unwrap_or(0);
    Ok(Json(serde_json::json!({ "count": c })))
}
