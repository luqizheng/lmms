use axum::{Router, routing::{get, post}, extract::{Json, Path, Query, State}};
use crate::{state::AppState, dto::{CreateUserRequest, UpdateUserRequest, UserQuery, UserResponse, UserListResponse, UserSimpleQuery, UserSimpleListResponse, UserClaimsResponse}, service::UserService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/users", get(list_users).post(create_user))
        .route("/iam/users/{user_id}", get(get_user).put(update_user).delete(delete_user))
        .route("/iam/users/simple", get(list_users_simple))
        .route("/internal/users/{user_id}/claims", get(get_user_claims))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/iam/users",
    params(UserQuery),
    responses((status = 200, body = UserListResponse)),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UserQuery>,
) -> Result<Json<UserListResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.list(query).await.map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/iam/users",
    request_body = CreateUserRequest,
    responses(
        (status = 200, body = UserResponse),
        (status = 409, description = "duplicate user"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateUser => (axum::http::StatusCode::CONFLICT, "duplicate user"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/iam/users/{user_id}",
    params(("user_id" = String, Path,)),
    responses(
        (status = 200, body = UserResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.get(&user_id).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => (axum::http::StatusCode::NOT_FOUND, "user not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/iam/users/{user_id}",
    params(("user_id" = String, Path,)),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, body = UserResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.update(&user_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => (axum::http::StatusCode::NOT_FOUND, "user not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/iam/users/{user_id}",
    params(("user_id" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    service.delete(&user_id).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => (axum::http::StatusCode::NOT_FOUND, "user not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/iam/users/simple",
    params(UserSimpleQuery),
    responses((status = 200, body = UserSimpleListResponse)),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn list_users_simple(
    State(state): State<AppState>,
    Query(query): Query<UserSimpleQuery>,
) -> Result<Json<UserSimpleListResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.list_simple(query).await.map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(response))
}

pub async fn get_user_claims(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserClaimsResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.get_claims(&user_id).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => (axum::http::StatusCode::NOT_FOUND, "user not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}