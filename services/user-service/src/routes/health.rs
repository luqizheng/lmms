use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::db::DbPool;

pub async fn health(State(pool): State<DbPool>) -> impl IntoResponse {
    let db_ok = match &pool {
        DbPool::Mysql(p) => sqlx::query("SELECT 1").execute(p).await.is_ok(),
    };
    let status_code = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = Json(json!({
        "status": if db_ok { "healthy" } else { "degraded" },
        "service": "user-service-rust",
        "components": { "database": if db_ok { "up" } else { "down" } }
    }));
    (status_code, body)
}
