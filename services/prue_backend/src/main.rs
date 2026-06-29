use common_db::init_pool;
use common_telemetry::init_tracing;
use prue_backend::{config::AppConfig, db, routes, state::AppState};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let config = AppConfig::from_env();
    init_tracing("prue_backend");

    tracing::info!("Starting prue_backend on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    if let Err(e) = db::ensure_schema(&pool).await {
        tracing::warn!(error = %e, "prue_backend schema initialization skipped");
    }

    let state = AppState::new(pool);

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
                .url("/api-docs/openapi.json", prue_backend::openapi::ApiDoc::openapi()),
        )
    } else {
        app
    };

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
