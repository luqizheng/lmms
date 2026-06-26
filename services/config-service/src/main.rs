use config_service::{config::AppConfig, routes::{self, create_state}};
use common_db::init_pool;
use common_log::init_json;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    init_json("info");

    let config = AppConfig::from_env();
    tracing::info!("Starting config-service on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    config_service::db::ensure_schema(&pool)
        .await
        .expect("ensure_schema failed");

    let state = create_state(pool);

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
                .url("/api-docs/openapi.json", config_service::openapi::ApiDoc::openapi()),
        )
    } else {
        app
    };

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");

    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    tracing::info!("config-service listening on {}", addr);

    axum::serve(listener, app).await.expect("Server error");
}
