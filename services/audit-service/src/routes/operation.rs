use axum::{extract::State, Json};
use crate::error::AppResult;
use crate::models::operation::{CreateOperationLog, QueryOperationLogs, PagedOperationLogs};
use crate::routes::AppState;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/operation", axum::routing::post(create_operation_log))
        .route("/operation", axum::routing::get(query_operation_logs))
        .with_state(state)
}

async fn create_operation_log(
    State(state): State<AppState>,
    Json(log): Json<CreateOperationLog>,
) -> AppResult<Json<serde_json::Value>> {
    let id = state.create_operation_log(log).await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn query_operation_logs(
    State(state): State<AppState>,
    Query(query): Query<QueryOperationLogs>,
) -> AppResult<Json<PagedOperationLogs>> {
    let result = state.query_operation_logs(query).await?;
    Ok(Json(result))
}

use axum::extract::Query;
