use axum::{extract::{Path, State}, Json, Router};
use crate::error::{AppError, AppResult};
use crate::models::RetentionStats;
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/retention/stats", axum::routing::get(get_retention_stats))
        .route("/retention/policy", axum::routing::get(get_retention_policy))
        .route("/retention/policy", axum::routing::put(update_retention_policy))
        .route("/retention/cleanup", axum::routing::post(trigger_cleanup))
        .route("/retention/stage/{created_at}", axum::routing::get(get_data_stage))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/audit/retention/stats",
    summary = "Get retention stats",
    responses(
        (status = 200, description = "Retention stats", body = RetentionStats),
    ),
    tag = "retention"
)]
pub async fn get_retention_stats(
    State(state): State<AppState>,
) -> AppResult<Json<RetentionStats>> {
    let stats = state.cleanup.get_stats().await?;
    Ok(Json(stats))
}

#[utoipa::path(
    get,
    path = "/audit/retention/policy",
    summary = "Get retention policy",
    responses(
        (status = 200, description = "Current retention policy", body = crate::models::RetentionPolicy),
    ),
    tag = "retention"
)]
pub async fn get_retention_policy(
    State(state): State<AppState>,
) -> AppResult<Json<crate::models::RetentionPolicy>> {
    let policy = state.cleanup.get_policy().await;
    Ok(Json(policy))
}

#[utoipa::path(
    put,
    path = "/audit/retention/policy",
    summary = "Update retention policy",
    request_body = crate::models::RetentionPolicy,
    responses(
        (status = 200, description = "Updated retention policy", body = crate::models::RetentionPolicy),
        (status = 400, description = "Invalid policy (hot < warm < cold required)"),
    ),
    tag = "retention"
)]
pub async fn update_retention_policy(
    State(state): State<AppState>,
    Json(policy): Json<crate::models::RetentionPolicy>,
) -> AppResult<Json<crate::models::RetentionPolicy>> {
    if policy.warm_days <= policy.hot_days {
        return Err(AppError::bad_request(
            "CFG_VALID_2003",
            "warm_days must be greater than hot_days"
        ));
    }
    
    if policy.cold_days <= policy.warm_days {
        return Err(AppError::bad_request(
            "CFG_VALID_2004", 
            "cold_days must be greater than warm_days"
        ));
    }

    state.cleanup.update_policy(policy.clone()).await;
    
    Ok(Json(policy))
}

#[utoipa::path(
    post,
    path = "/audit/retention/cleanup",
    summary = "Trigger retention cleanup",
    responses(
        (status = 200, description = "Cleanup result", body = serde_json::Value),
    ),
    tag = "retention"
)]
pub async fn trigger_cleanup(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let result = state.cleanup.cleanup_expired().await?;
    
    Ok(Json(serde_json::json!({
        "deleted_count": result.deleted_count,
        "cutoff_date": result.cutoff_date.to_rfc3339(),
        "success": true
    })))
}

#[utoipa::path(
    get,
    path = "/audit/retention/stage/{created_at}",
    summary = "Get data stage classification",
    params(("created_at" = String, Path, description = "RFC3339 timestamp")),
    responses(
        (status = 200, description = "Data stage classification", body = serde_json::Value),
        (status = 400, description = "Invalid date format"),
    ),
    tag = "retention"
)]
pub async fn get_data_stage(
    State(state): State<AppState>,
    Path(created_at): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    use chrono::{DateTime, Utc};
    
    let created = DateTime::parse_from_rfc3339(&created_at)
        .map_err(|_| AppError::bad_request("CFG_VALID_2005", "invalid date format"))?;
    
    let policy = state.cleanup.get_policy().await;
    let stage = policy.stage(created.with_timezone(&Utc));
    
    Ok(Json(serde_json::json!({
        "created_at": created_at,
        "stage": stage.as_str(),
        "policy": {
            "hot_days": policy.hot_days,
            "warm_days": policy.warm_days,
            "cold_days": policy.cold_days,
        }
    })))
}
