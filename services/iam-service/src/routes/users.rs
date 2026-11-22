use crate::{
    dto::{
        CreateUserRequest, UpdateSelfRequest, UpdateUserRequest, UserListResponse, UserQuery,
        UserResponse, UserSimpleListResponse, UserSimpleQuery,
    },
    service::UserService,
    state::AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    routing::get,
    Router,
};
use digua_common_error::AppError;
use digua_jwt_ctx::{Principal, TenantScope};
use validator::Validate;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/users", get(list_users).post(create_user))
        .route("/iam/users/me", get(get_me).put(update_me))
        .route(
            "/iam/users/{user_id}",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route("/iam/users/simple", get(list_users_simple))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/iam/users/me",
    responses(
        (status = 200, body = UserResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn get_me(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<Json<UserResponse>, AppError> {
    let scope = TenantScope::Single(principal.tenant_id);
    let service = UserService::new(state);
    let response = service
        .get(&principal.user_id, &scope)
        .await
        .map_err(AppError::from)?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/iam/users/me",
    request_body = UpdateSelfRequest,
    responses(
        (status = 200, body = UserResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub async fn update_me(
    State(state): State<AppState>,
    principal: Principal,
    Json(req): Json<UpdateSelfRequest>,
) -> Result<Json<UserResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::bad_request("IAM_SYS_9005", e.to_string()))?;
    let scope = TenantScope::Single(principal.tenant_id);

    // Self-service may only touch nickname and avatar. Email / phone changes
    // require a verification flow and are rejected here by construction.
    let update = UpdateUserRequest {
        nickname: req.nickname,
        email: None,
        phone: None,
        avatar: req.avatar,
        status: None,
    };

    let service = UserService::new(state);
    let response = service
        .update(&principal.user_id, update, &scope)
        .await
        .map_err(AppError::from)?;
    Ok(Json(response))
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
) -> Result<Json<UserResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::bad_request("IAM_SYS_9005", e.to_string()))?;

    let has_password = req.password.is_some();
    let send_invitation = req.send_invitation.unwrap_or(!has_password);
    if !has_password && !send_invitation {
        return Err(AppError::bad_request(
            "IAM_VALID_2006",
            "password required when invitation is disabled",
        ));
    }

    let service = UserService::new(state.clone());
    let response = service.create(req, &scope).await.map_err(AppError::from)?;

    if send_invitation && !has_password {
        crate::service::SelfService::new(state)
            .invite_user(&response.id)
            .await
            .map_err(AppError::from)?;
    }

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
