use crate::auth::Identity;
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
        .route("/announcements/:id", get(detail))
        .route("/announcements/:id/publish", post(publish))
        .route("/announcements/:id/revoke", post(revoke))
        .route("/announcements/:id/read", post(read_one))
        .route("/unread-count", get(unread_count))
        .with_state(state)
}

async fn create(
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

async fn publish(
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

async fn revoke(
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

async fn detail(
    State(s): State<RestState>,
    _me: Identity,
    Path(id): Path<u64>,
) -> AppResult<Json<Announcement>> {
    s.db.get(id).await?.map(Json).ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
struct ListQuery {
    pinned: Option<bool>,
    active: Option<bool>,
}

async fn list_mine(
    State(s): State<RestState>,
    me: Identity,
    _q: Query<ListQuery>,
) -> AppResult<Json<Vec<Announcement>>> {
    Ok(Json(s.db.list_for_user(me.user_id).await?))
}

async fn read_one(
    State(s): State<RestState>,
    me: Identity,
    Path(id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    let affected = s.db.mark_read(id, me.user_id).await?;
    if affected {
        s.redis.decr_unread_floor0(me.user_id).await.ok();
    }
    Ok(Json(serde_json::json!({ "ok": true, "changed": affected })))
}

async fn unread_count(
    State(s): State<RestState>,
    me: Identity,
) -> AppResult<Json<serde_json::Value>> {
    let c = s.redis.get_unread(me.user_id).await.unwrap_or(0);
    Ok(Json(serde_json::json!({ "count": c })))
}
