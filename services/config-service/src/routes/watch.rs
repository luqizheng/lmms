use axum::{extract::State, extract::Query};
use crate::routes::AppState;
use std::time::Duration;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/watch", axum::routing::get(watch_config))
        .route("/sse", axum::routing::get(sse_config))
        .with_state(state)
}

async fn watch_config(
    State(state): State<AppState>,
    Query(params): Query<WatchParams>,
) -> axum::response::Json<serde_json::Value> {
    let namespace_id = params.namespace_id.unwrap_or_default();
    let timeout = params.timeout.unwrap_or(30);
    
    tokio::time::sleep(Duration::from_secs(timeout)).await;
    
    axum::response::Json(serde_json::json!({
        "namespace_id": namespace_id,
        "changed": false,
        "data": []
    }))
}

async fn sse_config(
    State(state): State<AppState>,
    Query(params): Query<WatchParams>,
) -> axum::response::Sse<axum::response::sse::Event> {
    let namespace_id = params.namespace_id.unwrap_or_default();
    
    axum::response::Sse::new(async_stream::stream! {
        yield axum::response::sse::Event::default().data("connected");
        
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            yield axum::response::sse::Event::default().data(format!("ping: {}", namespace_id));
        }
    })
}

#[derive(Debug, serde::Deserialize)]
struct WatchParams {
    namespace_id: Option<String>,
    timeout: Option<u64>,
}
