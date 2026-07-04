use axum::{
    Router,
    routing::{get, post, put, delete},
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    Json as AxumJson,
};
use crate::{state::AppState, dto::{LoginRequest, RefreshRequest, ChangePasswordRequest, LoginResponse, CreateUserRequest, UserResponse}, service::AuthService, error::IamError};
use jwt_ctx::Principal;
use sqlx;
use serde_json;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/auth/login", post(login))
        .route("/iam/auth/refresh", post(refresh))
        .route("/iam/auth/logout", post(logout))
        .route("/iam/auth/password", post(change_password))
        .route("/iam/auth/password", put(change_password))
        .route("/iam/auth/sessions", get(list_sessions))
        .route("/iam/auth/sessions/{session_id}", delete(revoke_session))
        .route("/iam/auth/register", post(register))
        .route("/iam/auth/setup-status", get(setup_status))
        .with_state(state)
}

fn get_client_info(headers: &HeaderMap) -> (String, String) {
    let ip_address = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let user_agent = headers
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    (ip_address, user_agent)
}

#[utoipa::path(
    post,
    path = "/iam/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 401, description = "invalid credentials"),
    ),
    tag = "auth"
)]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(login_req): Json<LoginRequest>,
) -> Result<AxumJson<LoginResponse>, (StatusCode, &'static str)> {
    let (ip_address, user_agent) = get_client_info(&headers);
    let service = AuthService::new(state);
    let response = service
        .login(login_req, &ip_address, &user_agent)
        .await
        .map_err(|e| {
            tracing::error!("login failed: {:?}", e);
            (StatusCode::UNAUTHORIZED, "invalid credentials")
        })?;
    Ok(AxumJson(response))
}

#[utoipa::path(
    post,
    path = "/iam/auth/refresh",
    request_body = RefreshRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 401, description = "token expired"),
    ),
    tag = "auth"
)]
pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<AxumJson<LoginResponse>, (StatusCode, &'static str)> {
    let service = AuthService::new(state);
    let response = service
        .refresh(&req.refresh_token)
        .await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "token expired"))?;
    Ok(AxumJson(response))
}

#[utoipa::path(
    post,
    path = "/iam/auth/logout",
    responses((status = 204, description = "logged out")),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub async fn logout(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let service = AuthService::new(state);
    service
        .logout(&principal.user_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/iam/auth/password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "password changed"),
        (status = 401, description = "invalid credentials"),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub async fn change_password(
    State(state): State<AppState>,
    principal: Principal,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let service = AuthService::new(state);
    service
        .change_password(&principal.user_id, req)
        .await
        .map_err(|e| {
            match e {
                IamError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid credentials"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
            }
        })?;
    Ok(StatusCode::OK)
}

#[utoipa::path(
    get,
    path = "/iam/auth/sessions",
    responses((status = 200, body = [crate::dto::SessionInfo])),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub async fn list_sessions(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<AxumJson<Vec<crate::dto::SessionInfo>>, (StatusCode, &'static str)> {
    let service = AuthService::new(state);
    let sessions = service
        .get_user_sessions(&principal.user_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    let session_infos: Vec<crate::dto::SessionInfo> = sessions
        .into_iter()
        .map(|t| crate::dto::SessionInfo {
            id: t.id,
            user_id: t.user_id,
            client_ip: t.client_ip,
            user_agent: t.user_agent,
            created_at: t.created_at.to_rfc3339(),
            expires_at: t.expires_at.to_rfc3339(),
            is_current: false,
        })
        .collect();

    Ok(AxumJson(session_infos))
}

async fn revoke_session(
    State(state): State<AppState>,
    principal: Principal,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let _ = principal;
    let service = AuthService::new(state);
    service
        .revoke_session(&session_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, AxumJson<UserResponse>), (StatusCode, &'static str)> {
    let user_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_users WHERE deleted_at IS NULL"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("count users failed: {:?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    if user_count.0 > 0 {
        return Err((StatusCode::FORBIDDEN, "registration closed"));
    }

    let service = crate::service::UserService::new(state);
    let response = service
        .create(req, 1i64)
        .await
        .map_err(|e| {
            tracing::error!("create user failed: {:?}", e);
            match e {
                IamError::DuplicateUser => (StatusCode::CONFLICT, "username already exists"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
            }
        })?;
    Ok((StatusCode::CREATED, AxumJson(response)))
}

async fn setup_status(
    State(state): State<AppState>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, &'static str)> {
    let user_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_users WHERE deleted_at IS NULL"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    Ok(AxumJson(serde_json::json!({
        "setup_required": user_count.0 == 0,
        "user_count": user_count.0,
    })))
}