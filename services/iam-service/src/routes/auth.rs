use axum::{
    Router,
    routing::{get, post, delete},
    extract::{ConnectInfo, Json, Path, State},
    http::{HeaderMap, StatusCode},
};
use std::net::SocketAddr;
use crate::{state::AppState, dto::{LoginRequest, RefreshRequest, ChangePasswordRequest, LoginResponse}, service::AuthService};
use jwt_ctx::Principal;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/auth/login", post(login))
        .route("/iam/auth/refresh", post(refresh))
        .route("/iam/auth/logout", post(logout))
        .route("/iam/auth/password", post(change_password))
        .route("/iam/auth/sessions", get(list_sessions))
        .route("/iam/auth/sessions/:session_id", delete(revoke_session))
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

async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(login_req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, &'static str)> {
    let (ip_address, user_agent) = get_client_info(&headers);
    let _ = addr;
    let service = AuthService::new(state);
    let response = service
        .login(login_req, &ip_address, &user_agent)
        .await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid credentials"))?;
    Ok(Json(response))
}

async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, &'static str)> {
    let service = AuthService::new(state);
    let response = service
        .refresh(&req.refresh_token)
        .await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "token expired"))?;
    Ok(Json(response))
}

async fn logout(
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

async fn change_password(
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
                crate::error::IamError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid credentials"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
            }
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_sessions(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<axum::Json<Vec<crate::dto::SessionInfo>>, (StatusCode, &'static str)> {
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

    Ok(axum::Json(session_infos))
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
