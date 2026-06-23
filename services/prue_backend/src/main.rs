use common_db::init_pool;
use common_log::init_json;
use prue_backend::{config::AppConfig, db, routes, state::AppState};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let config = AppConfig::from_env();
    init_json(&config.log_level);

    tracing::info!("Starting prue_backend on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    if let Err(e) = db::ensure_schema(&pool).await {
        tracing::warn!(error = %e, "prue_backend schema initialization skipped");
    }

    let state = AppState::new(pool);
    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, routes::routes(state))
        .await
        .expect("Server error");
}
