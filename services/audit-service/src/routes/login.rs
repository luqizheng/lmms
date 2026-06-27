use axum::{extract::State, Json, extract::Query, Router};
use crate::error::AppResult;
use crate::models::login::{QueryLoginLogs, PagedLoginLogs};
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/login", axum::routing::get(query_login_logs))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/audit/login",
    params(QueryLoginLogs),
    responses(
        (status = 200, description = "Paged login logs", body = PagedLoginLogs),
    ),
    tag = "login"
)]
pub async fn query_login_logs(
    State(state): State<AppState>,
    Query(query): Query<QueryLoginLogs>,
) -> AppResult<Json<PagedLoginLogs>> {
    let result = state.audit.query_login_logs(query).await?;
    Ok(Json(result))
}
