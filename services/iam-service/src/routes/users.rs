use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, Query, State}};
use crate::{state::AppState, dto::{CreateUserRequest, UpdateUserRequest, UserQuery, UserResponse, UserListResponse}, service::UserService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/users", get(list_users).post(create_user))
        .route("/iam/users/:user_id", get(get_user).put(update_user).delete(delete_user))
        .with_state(state)
}

async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UserQuery>,
) -> Result<Json<UserListResponse>, axum::http::StatusCode> {
    let service = UserService::new(state);
    let response = service.list(query).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, axum::http::StatusCode> {
    let service = UserService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateUser => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserResponse>, axum::http::StatusCode> {
    let service = UserService::new(state);
    let response = service.get(&user_id).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, axum::http::StatusCode> {
    let service = UserService::new(state);
    let response = service.update(&user_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = UserService::new(state);
    service.delete(&user_id).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}
