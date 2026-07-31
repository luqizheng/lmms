use digua_common_db::init_pool;
use digua_common_jwt::{JWKSCache, JwksConfig};
use digua_common_telemetry::init_tracing;
use prue_backend::{config::AppConfig, db, routes, state::AppState};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::from_env()?;
    init_tracing("prue_backend");

    tracing::info!("Starting prue_backend on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    if let Err(e) = db::ensure_schema(&pool).await {
        tracing::warn!(error = %e, "prue_backend schema initialization skipped");
    }

    let http_for_jwt = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build reqwest client for JWKS");
    let jwks = Arc::new(JWKSCache::new(
        JwksConfig {
            url: std::env::var("JWKS_URL")
                .unwrap_or_else(|_| "http://token-service:8082/.well-known/jwks.json".into()),
            refresh_interval: Duration::from_secs(3600),
            request_timeout: Duration::from_secs(5),
        },
        http_for_jwt,
    ));
    // Block up to 30s on startup retry + spawn periodic refresh loop.
    // Fixes docker-compose boot race: token-service may not be listening yet
    // when this service comes up; previously a single failed `refresh()` was
    // warn-ignored and the cache stayed empty forever (401 on every verify).
    let jwks_shutdown = Arc::clone(&jwks)
        .init_with_refresh_loop(Duration::from_secs(30))
        .await;

    let state = AppState::new(pool, jwks);

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

    let consul_addr = std::env::var("CONSUL_ADDR")
        .unwrap_or_else(|_| "http://consul:8500".to_string());
        tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        tracing::info!("received shutdown signal, deregistering from consul");
        let _ = jwks_shutdown.send(true);
        std::process::exit(0);
    });

    axum::serve(listener, app)
        .await
        .expect("Server error");
    Ok(())
}
