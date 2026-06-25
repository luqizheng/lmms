use axum::{extract::State, Json, extract::Query, Router};
use crate::error::AppResult;
use crate::models::operation::{CreateOperationLog, QueryOperationLogs, PagedOperationLogs};
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/operation", axum::routing::post(create_operation_log))
        .route("/operation", axum::routing::get(query_operation_logs))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/audit/operation",
    request_body = CreateOperationLog,
    responses(
        (status = 200, description = "Operation log created", body = serde_json::Value),
    ),
    tag = "operation"
)]
pub async fn create_operation_log(
    State(state): State<AppState>,
    Json(log): Json<CreateOperationLog>,
) -> AppResult<Json<serde_json::Value>> {
    let id = state.audit.create_operation_log(log).await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

#[utoipa::path(
    get,
    path = "/audit/operation",
    params(QueryOperationLogs),
    responses(
        (status = 200, description = "Paged operation logs", body = PagedOperationLogs),
    ),
    tag = "operation"
)]
pub async fn query_operation_logs(
    State(state): State<AppState>,
    Query(query): Query<QueryOperationLogs>,
) -> AppResult<Json<PagedOperationLogs>> {
    let result = state.audit.query_operation_logs(query).await?;
    Ok(Json(result))
}
