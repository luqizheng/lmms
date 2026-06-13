use axum::{Router, routing::{post, delete}, extract::{Json, State}};
use crate::{state::AppState, dto::{LoginRequest, RefreshRequest, ChangePasswordRequest, LoginResponse}, service::AuthService};
use jwt_ctx::Principal;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/auth/login", post(login))
        .route("/iam/auth/refresh", post(refresh))
        .route("/iam/auth/logout", delete(logout))
        .route("/iam/auth/password", post(change_password))
        .with_state(state)
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (axum::http::StatusCode, &'static str)> {
    let service = AuthService::new(state);
    let response = service.login(req).await.map_err(|_| (axum::http::StatusCode::UNAUTHORIZED, "invalid credentials"))?;
    Ok(Json(response))
}

async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<LoginResponse>, (axum::http::StatusCode, &'static str)> {
    let service = AuthService::new(state);
    let response = service.refresh(&req.refresh_token).await.map_err(|_| (axum::http::StatusCode::UNAUTHORIZED, "token expired"))?;
    Ok(Json(response))
}

async fn logout(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = AuthService::new(state);
    service.logout(&principal.user_id).await.map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn change_password(
    State(state): State<AppState>,
    principal: Principal,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = AuthService::new(state);
    service.change_password(&principal.user_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::InvalidCredentials => (axum::http::StatusCode::UNAUTHORIZED, "invalid credentials"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}