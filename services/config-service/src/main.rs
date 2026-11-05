use config_service::{
    config::AppConfig,
    db::{ensure_schema, init_pool},
    routes::{self, create_state},
};
use digua_common_jwt::{JWKSCache, JwksConfig};
use digua_common_telemetry::init_tracing;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("config-service");

    let config = AppConfig::from_env()?;
    tracing::info!("Starting config-service on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    ensure_schema(&pool).await.expect("ensure_schema failed");

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
    // Block up to 30s on startup retry + spawn periodic refresh loop.
    // Fixes docker-compose boot race: token-service may not be listening yet
    // when this service comes up; previously a single failed `refresh()` was
    // warn-ignored and the cache stayed empty forever (401 on every verify).
    let jwks_shutdown = Arc::clone(&jwks)
        .init_with_refresh_loop(Duration::from_secs(30))
        .await;

    let state = create_state(pool, jwks);

    let app = routes::routes(state);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false);
    let app = if swagger_enabled {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        tracing::info!("swagger ui enabled at /swagger-ui");
        app.merge(SwaggerUi::new("/swagger-ui").url(
            "/api-docs/openapi.json",
            config_service::openapi::ApiDoc::openapi(),
        ))
    } else {
        app
    };

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    tracing::info!("config-service listening on {}", addr);

    // 自注册（libs/common-consul）：SERVICE_NAME 未设时为 no-op。
    let consul_guard = digua_common_consul::consul_guard();
    tokio::spawn(async move {
        // SIGINT + SIGTERM（docker stop 走 SIGTERM；旧代码只听 ctrl_c，
        // SIGTERM 无人消费 → 超时 SIGKILL，注销路径从未真正执行）。
        digua_common_consul::wait_for_shutdown_signal().await;
        tracing::info!("received shutdown signal");
        let _ = jwks_shutdown.send(true);
        consul_guard.deregister().await;
        std::process::exit(0);
    });

    axum::serve(listener, app).await.expect("Server error");
    Ok(())
}
