use audit_service::{config::AppConfig, routes, create_state};
use common_db::init_pool;
use common_log::init_json;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    init_json("info");

    let config = AppConfig::from_env();
    tracing::info!("Starting audit-service on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    let state = create_state(pool);

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");
    
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    tracing::info!("audit-service listening on {}", addr);

    let app = routes(state);
    
    axum::serve(listener, app).await.expect("Server error");
}
