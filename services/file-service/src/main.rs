use file_service::{config::AppConfig, routes::{self, create_state}, storage::LocalStorage};
use common_db::init_pool;
use common_telemetry::init_tracing;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    init_tracing("file-service");

    let config = AppConfig::from_env();
    tracing::info!("Starting file-service on {}", config.server_addr);

    std::fs::create_dir_all(&config.storage_path).expect("Failed to create storage directory");

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    file_service::db::ensure_schema(&pool)
        .await
        .expect("ensure_schema failed");

    let storage = Arc::new(LocalStorage::new(&config.storage_path));
    let base_url = format!("http://localhost:8082/file/");

    let state = create_state(pool, storage, base_url);

    let app = routes::routes(state);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false);
    let app = if swagger_enabled {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        tracing::info!("swagger ui enabled at /swagger-ui");
        app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", file_service::openapi::ApiDoc::openapi()),
        )
    } else {
        app
    };

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");

    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    tracing::info!("file-service listening on {}", addr);

    axum::serve(listener, app).await.expect("Server error");
}
