use crate::config::Config;
use crate::dto::{
    ErrorResp, InvalidCredsResp, LockedResp, LoginReq, LoginSuccessResp, RegisterReq,
    RegisterResp, SetupStatusResp,
};
use crate::error::AppError;
use crate::service::auth_svc::{self, LoginCtx, LoginOutcome, RegisterError};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router, routing};
use std::net::SocketAddr;

pub async fn login(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AuthState>,
    Json(req): Json<LoginReq>,
) -> Result<axum::response::Response, AppError> {
    if req.username.is_empty() || req.password.is_empty() {
        return Err(AppError::Validation(
            "username and password are required".into(),
        ));
    }
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let ip = client_ip(&headers, addr);

    let ctx = LoginCtx {
        pool: &state.pool,
        redis: &state.redis,
        cfg: &state.cfg.login_security,
        ip,
        user_agent: ua,
    };
    let outcome = auth_svc::login(&ctx, &req.username, &req.password).await?;
    Ok(match outcome {
        LoginOutcome::Success { code, .. } => (
            StatusCode::OK,
            Json(LoginSuccessResp {
                code,
                expires_in: state.cfg.login_security.code_ttl.as_secs() as u32,
            }),
        )
            .into_response(),
        LoginOutcome::Locked { retry_after_secs } => (
            StatusCode::LOCKED,
            Json(LockedResp {
                error: "account locked",
                retry_after: retry_after_secs,
            }),
        )
            .into_response(),
        LoginOutcome::Invalid => (
            StatusCode::UNAUTHORIZED,
            Json(InvalidCredsResp {
                error: "invalid credentials",
            }),
        )
            .into_response(),
    })
}

pub async fn register(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AuthState>,
    Json(req): Json<RegisterReq>,
) -> Result<axum::response::Response, AppError> {
    let _ip = client_ip(&headers, addr);
    let _ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let res = auth_svc::register_admin(
        &state.pool,
        &req.username,
        &req.password,
        req.email.clone(),
        req.phone.clone(),
        req.nickname.clone(),
    )
    .await;
    Ok(match res {
        Ok(uid) => (
            StatusCode::CREATED,
            Json(RegisterResp {
                id: uid,
                username: req.username,
            }),
        )
            .into_response(),
        Err(RegisterError::Closed) => (
            StatusCode::FORBIDDEN,
            Json(ErrorResp {
                error: "registration closed".into(),
            }),
        )
            .into_response(),
        Err(RegisterError::UsernameTaken) => (
            StatusCode::CONFLICT,
            Json(ErrorResp {
                error: "username taken".into(),
            }),
        )
            .into_response(),
        Err(RegisterError::InvalidInput(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp { error: msg }),
        )
            .into_response(),
        Err(RegisterError::Internal(msg)) => {
            tracing::error!(error = %msg, "register_admin failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResp {
                    error: "internal".into(),
                }),
            )
                .into_response()
        }
    })
}

pub async fn setup_status(
    State(state): State<AuthState>,
) -> Result<Json<SetupStatusResp>, AppError> {
    let required = auth_svc::setup_status(&state.pool).await?;
    Ok(Json(SetupStatusResp {
        setup_required: required,
    }))
}

fn client_ip(headers: &HeaderMap, addr: SocketAddr) -> String {
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            return first.trim().to_string();
        }
    }
    addr.ip().to_string()
}

#[derive(Clone)]
pub struct AuthState {
    pub pool: crate::db::DbPool,
    pub redis: redis::Client,
    pub cfg: Config,
}

pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/api/v1/auth/login", routing::post(login))
        .route("/api/v1/auth/register", routing::post(register))
        .route("/api/v1/auth/setup-status", routing::get(setup_status))
        .merge(crate::routes::org_units::router().with_state(state.pool.clone()))
        .merge(crate::routes::positions::router().with_state(state.pool.clone()))
        .merge(crate::routes::user_positions::router().with_state(state.pool.clone()))
        .merge(crate::routes::users::router())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn client_ip_prefers_x_forwarded_for() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            HeaderValue::from_static("10.0.0.1, 10.0.0.2"),
        );
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        assert_eq!(client_ip(&h, addr), "10.0.0.1");
    }

    #[test]
    fn client_ip_falls_back_to_socket() {
        let h = HeaderMap::new();
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        assert_eq!(client_ip(&h, addr), "127.0.0.1");
    }
}
