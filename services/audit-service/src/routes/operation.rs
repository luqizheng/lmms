use axum::{extract::State, Json, extract::Query, Router};
use crate::error::AppResult;
use crate::models::operation::{QueryOperationLogs, PagedOperationLogs};
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/operation", axum::routing::get(query_operation_logs))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/audit/operation",
    summary = "Query operation logs",
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
