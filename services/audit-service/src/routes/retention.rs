use axum::{extract::{Path, State}, Json, Router};
use crate::error::AppResult;
use crate::models::RetentionStats;
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/retention/stats", axum::routing::get(get_retention_stats))
        .route("/retention/policy", axum::routing::get(get_retention_policy))
        .route("/retention/policy", axum::routing::put(update_retention_policy))
        .route("/retention/cleanup", axum::routing::post(trigger_cleanup))
        .route("/retention/stage/:created_at", axum::routing::get(get_data_stage))
        .with_state(state)
}

pub async fn get_retention_stats(
    State(state): State<AppState>,
) -> AppResult<Json<RetentionStats>> {
    let stats = state.cleanup.get_stats().await?;
    Ok(Json(stats))
}

pub async fn get_retention_policy(
    State(state): State<AppState>,
) -> AppResult<Json<crate::models::RetentionPolicy>> {
    Ok(Json(state.policy.clone()))
}

pub async fn update_retention_policy(
    State(state): State<AppState>,
    Json(policy): Json<crate::models::RetentionPolicy>,
) -> AppResult<Json<crate::models::RetentionPolicy>> {
    // 验证策略
    if policy.warm_days <= policy.hot_days {
        return Err(crate::error::AppError::bad_request(
            "CFG_VALID_2003",
            "warm_days must be greater than hot_days"
        ));
    }
    
    if policy.cold_days <= policy.warm_days {
        return Err(crate::error::AppError::bad_request(
            "CFG_VALID_2004", 
            "cold_days must be greater than warm_days"
        ));
    }

    // 更新策略
    state.policy = policy.clone();
    
    Ok(Json(policy))
}

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

pub async fn get_data_stage(
    State(state): State<AppState>,
    Path(created_at): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    use chrono::{DateTime, Utc};
    
    let created = DateTime::parse_from_rfc3339(&created_at)
        .map_err(|_| crate::error::AppError::bad_request("CFG_VALID_2005", "invalid date format"))?;
    
    let stage = state.policy.stage(created.with_timezone(&Utc));
    
    Ok(Json(serde_json::json!({
        "created_at": created_at,
        "stage": stage.as_str(),
        "policy": {
            "hot_days": state.policy.hot_days,
            "warm_days": state.policy.warm_days,
            "cold_days": state.policy.cold_days,
        }
    })))
}
