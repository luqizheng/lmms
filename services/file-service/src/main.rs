use file_service::{config::AppConfig, routes::{self, create_state}, storage::LocalStorage};
use common_db::init_pool;
use common_log::init_json;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    init_json("info");

    let config = AppConfig::from_env();
    tracing::info!("Starting file-service on {}", config.server_addr);

    // Initialize storage directory
    std::fs::create_dir_all(&config.storage_path).expect("Failed to create storage directory");

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    let storage = Arc::new(LocalStorage::new(&config.storage_path));
    let base_url = format!("http://localhost:8082/file/");
    
    let state = create_state(pool, storage, base_url);

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");
    
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    tracing::info!("file-service listening on {}", addr);

    let app = routes::routes(state);
    
    axum::serve(listener, app).await.expect("Server error");
}