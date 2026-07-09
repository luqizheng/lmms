use config_service::{config::AppConfig, db::{ensure_schema, init_pool}, routes::{self, create_state}};
use common_jwt::{JWKSCache, JwksConfig};
use common_telemetry::init_tracing;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    init_tracing("config-service");

    let config = AppConfig::from_env();
    tracing::info!("Starting config-service on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    ensure_schema(&pool)
        .await
        .expect("ensure_schema failed");

    // Phase 5-B: JWKS cache.
    let http_for_jwt = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("reqwest");
    let jwks = Arc::new(JWKSCache::new(
        JwksConfig {
            url: std::env::var("JWKS_URL")
                .unwrap_or_else(|_| "http://token-service:8082/.well-known/jwks.json".into()),
            refresh_interval: Duration::from_secs(3600),
            request_timeout: Duration::from_secs(5),
        },
        http_for_jwt,
    ));
    // 降级为 warn：token-service 可能还在起；首个 service-jwt verify 时 JWKSCache 会自动 retry。
    if let Err(e) = jwks.refresh().await {
        tracing::warn!(
            "JWKS initial refresh failed (will retry on first verify): {e}; token-service must be reachable"
        );
    }

    let state = create_state(pool, jwks);

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

    let listener = tokio::net::TcpListener::bind(&addr).await.expect("Failed to bind");
    tracing::info!("config-service listening on {}", addr);

    axum::serve(listener, app).await.expect("Server error");
}
