use crate::{
    dto::{
        CreateUserRequest, UpdateUserRequest, UserClaimsResponse, UserListResponse, UserQuery,
        UserResponse, UserSimpleListResponse, UserSimpleQuery,
    },
    service::UserService,
    state::AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    routing::{get, post},
    Router,
};
use digua_jwt_ctx::TenantScope;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/users", get(list_users).post(create_user))
        .route(
            "/iam/users/{user_id}",
            get(get_user).put(update_user).delete(delete_user),
        )
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
// check-tenant-scope: delegated
pub async fn list_users(
    State(state): State<AppState>,
    scope: TenantScope,
    Query(query): Query<UserQuery>,
) -> Result<Json<UserListResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.list(query, &scope).await.map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal error",
        )
    })?;
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
// check-tenant-scope: delegated
pub async fn create_user(
    State(state): State<AppState>,
    scope: TenantScope,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.create(req, &scope).await.map_err(|e| match e {
        crate::error::IamError::DuplicateUser => {
            (axum::http::StatusCode::CONFLICT, "duplicate user")
        }
        _ => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal error",
        ),
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
// check-tenant-scope: delegated
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    scope: TenantScope,
) -> Result<Json<UserResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.get(&user_id, &scope).await.map_err(|e| match e {
        crate::error::IamError::UserNotFound => {
            (axum::http::StatusCode::NOT_FOUND, "user not found")
        }
        _ => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal error",
        ),
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
// check-tenant-scope: delegated
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    scope: TenantScope,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service
        .update(&user_id, req, &scope)
        .await
        .map_err(|e| match e {
            crate::error::IamError::UserNotFound => {
                (axum::http::StatusCode::NOT_FOUND, "user not found")
            }
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ),
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
// check-tenant-scope: delegated
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    scope: TenantScope,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    service
        .delete(&user_id, &scope)
        .await
        .map_err(|e| match e {
            crate::error::IamError::UserNotFound => {
                (axum::http::StatusCode::NOT_FOUND, "user not found")
            }
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ),
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
// check-tenant-scope: delegated
pub async fn list_users_simple(
    State(state): State<AppState>,
    scope: TenantScope,
    Query(query): Query<UserSimpleQuery>,
) -> Result<Json<UserSimpleListResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service.list_simple(query, &scope).await.map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal error",
        )
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/internal/users/{user_id}/claims",
    params(("user_id" = String, Path,)),
    responses(
        (status = 200, body = UserClaimsResponse),
        (status = 404, description = "user not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal"
)]
// check-tenant-scope: delegated
pub async fn get_user_claims(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    scope: TenantScope,
) -> Result<Json<UserClaimsResponse>, (axum::http::StatusCode, &'static str)> {
    let service = UserService::new(state);
    let response = service
        .get_claims(&user_id, &scope)
        .await
        .map_err(|e| match e {
            crate::error::IamError::UserNotFound => {
                (axum::http::StatusCode::NOT_FOUND, "user not found")
            }
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ),
        })?;
    Ok(Json(response))
}
