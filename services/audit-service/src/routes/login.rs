use axum::{extract::State, Json, extract::Query, Router};
use crate::error::AppResult;
use crate::models::login::{CreateLoginLog, QueryLoginLogs, PagedLoginLogs};
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/login", axum::routing::post(create_login_log))
        .route("/login", axum::routing::get(query_login_logs))
        .with_state(state)
}

async fn create_login_log(
    State(state): State<AppState>,
    Json(log): Json<CreateLoginLog>,
) -> AppResult<Json<serde_json::Value>> {
    let id = state.audit.create_login_log(log).await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn query_login_logs(
    State(state): State<AppState>,
    Query(query): Query<QueryLoginLogs>,
) -> AppResult<Json<PagedLoginLogs>> {
    let result = state.audit.query_login_logs(query).await?;
    Ok(Json(result))
}
