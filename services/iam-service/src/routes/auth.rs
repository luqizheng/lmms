use axum::{Router, routing::{post, delete}, extract::{Json, State}};
use crate::{state::AppState, dto::{LoginRequest, RefreshRequest, ChangePasswordRequest, LoginResponse, RefreshResponse}, service::AuthService};
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
) -> Result<Json<LoginResponse>, axum::http::StatusCode> {
    let service = AuthService::new(state);
    let response = service.login(req).await.map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;
    Ok(Json(response))
}

async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, axum::http::StatusCode> {
    let service = AuthService::new(state);
    let response = service.refresh(&req.refresh_token).await.map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;
    Ok(Json(response))
}

async fn logout(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<(), axum::http::StatusCode> {
    let service = AuthService::new(state);
    service.logout(&principal.user_id, None).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn change_password(
    State(state): State<AppState>,
    principal: Principal,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<(), axum::http::StatusCode> {
    let service = AuthService::new(state);
    service.change_password(&principal.user_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::InvalidCredentials => axum::http::StatusCode::UNAUTHORIZED,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}
