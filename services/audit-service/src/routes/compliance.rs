use crate::error::{AppError, AppResult};
use crate::routes::AppState;
use axum::{
    extract::{Path, State},
    http::header,
    Router,
};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/export/compliance/operation/csv/{user_id}",
            axum::routing::get(export_operation_csv),
        )
        .route(
            "/export/compliance/operation/json/{user_id}",
            axum::routing::get(export_operation_json),
        )
        .route(
            "/export/compliance/login/csv/{user_id}",
            axum::routing::get(export_login_csv),
        )
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/audit/export/compliance/operation/csv/{user_id}",
    summary = "Export operation logs as CSV",
    params(
        ("user_id" = String, Path, description = "User id to export operation logs for"),
    ),
    responses(
        (status = 200, description = "CSV export of operation logs", body = String),
    ),
    tag = "compliance"
)]
pub async fn export_operation_csv(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> AppResult<impl axum::response::IntoResponse> {
    let logs = state.audit.get_user_operation_logs(&user_id, 10000).await?;

    let mut csv =
        String::from("id,user_id,action,resource,resource_id,request_id,ip_address,created_at\n");
    for log in logs {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            log.id,
            log.user_id,
            log.action,
            log.resource,
            log.resource_id.unwrap_or_default(),
            log.request_id,
            log.ip_address.unwrap_or_default(),
            log.created_at.to_rfc3339()
        ));
    }

    Ok(([(header::CONTENT_TYPE, "text/csv; charset=utf-8")], csv))
}

#[utoipa::path(
    get,
    path = "/audit/export/compliance/operation/json/{user_id}",
    summary = "Export operation logs as JSON",
    params(
        ("user_id" = String, Path, description = "User id to export operation logs for"),
    ),
    responses(
        (status = 200, description = "JSON export of operation logs", body = String),
    ),
    tag = "compliance"
)]
pub async fn export_operation_json(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> AppResult<impl axum::response::IntoResponse> {
    let logs = state.audit.get_user_operation_logs(&user_id, 10000).await?;
    let json = serde_json::to_string_pretty(&logs)
        .map_err(|e| AppError::internal("EXPORT_JSON", e.to_string()))?;

    Ok(([(header::CONTENT_TYPE, "application/json")], json))
}

#[utoipa::path(
    get,
    path = "/audit/export/compliance/login/csv/{user_id}",
    summary = "Export login logs as CSV",
    params(
        ("user_id" = String, Path, description = "User id to export login logs for"),
    ),
    responses(
        (status = 200, description = "CSV export of login logs", body = String),
    ),
    tag = "compliance"
)]
pub async fn export_login_csv(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> AppResult<impl axum::response::IntoResponse> {
    let logs = state.audit.get_user_login_logs(&user_id, 10000).await?;

    let mut csv = String::from("id,user_id,username,success,fail_reason,ip_address,location,user_agent,request_id,created_at\n");
    for log in logs {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            log.id,
            log.user_id.unwrap_or_default(),
            log.username,
            log.success,
            log.fail_reason.unwrap_or_default(),
            log.ip_address,
            log.location.unwrap_or_default(),
            log.user_agent.unwrap_or_default(),
            log.request_id,
            log.created_at.to_rfc3339()
        ));
    }

    Ok(([(header::CONTENT_TYPE, "text/csv; charset=utf-8")], csv))
}
