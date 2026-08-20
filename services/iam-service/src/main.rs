use digua_common_cache::MemoryCache;
use digua_common_event::MySqlOutboxRepo;
use digua_common_jwt::{JWKSCache, JwksConfig, ServiceJwtCache, ServiceJwtConfig};
use iam_service::{
    config::load_config, db, openapi::ApiDoc, routes, service::TokenForwardClient, state::AppState,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    digua_common_telemetry::init_tracing("iam-service");

    let config = load_config()?;
    info!("Loaded config: {:?}", config.app.name);

    let db_pool = db::connect(&config.database).await?;
    let outbox_repo = Arc::new(MySqlOutboxRepo::new(db_pool.clone()));
    let cache = MemoryCache::new(10000);

    let redis = match redis::Client::open(config.redis.url.clone()) {
        Ok(client) => match redis::aio::ConnectionManager::new(client).await {
            Ok(c) => {
                info!("iam connected to redis for perm_epoch");
                Some(c)
            }
            Err(e) => {
                warn!("iam redis connect failed (perm_epoch direct-write disabled, gateway consumer will backfill): {e}");
                None
            }
        },
        Err(e) => {
            warn!("iam redis url invalid: {e}");
            None
        }
    };

    // Phase 5-B: JWKS cache + service-jwt cache.
    let http_for_jwt = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let jwks = Arc::new(JWKSCache::new(
        JwksConfig {
            url: std::env::var("JWKS_URL")
                .unwrap_or_else(|_| "http://token-service:8082/.well-known/jwks.json".into()),
            refresh_interval: Duration::from_secs(3600),
            request_timeout: Duration::from_secs(5),
        },
        http_for_jwt.clone(),
    ));
    // Block up to 30s on startup retry + spawn periodic refresh loop.
    // Fixes docker-compose boot race: token-service may not be listening yet
    // when iam-service comes up; previously a single failed `refresh()` was
    // warn-ignored and the cache stayed empty forever (401 on every verify).
    let jwks_shutdown = Arc::clone(&jwks)
        .init_with_refresh_loop(Duration::from_secs(30))
        .await;
    let service_jwt = Arc::new(ServiceJwtCache::new(
        ServiceJwtConfig {
            token_service_url: std::env::var("TOKEN_SERVICE_URL")
                .unwrap_or_else(|_| "http://token-service:8082".into()),
            bootstrap_token: std::env::var("SERVICE_BOOTSTRAP_TOKEN")
                .expect("SERVICE_BOOTSTRAP_TOKEN required"),
            caller_service_name: "iam-service".into(),
            caller_tenant: std::env::var("TENANT_ID")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1i64),
            refresh_skew: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        },
        http_for_jwt,
        std::env::var("IAM_PRIVATE_KEY_PEM_PATH")
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default(), // not used for self-mint
    ));
    let token_forward = Arc::new(TokenForwardClient::new(
        service_jwt.clone(),
        std::env::var("TOKEN_SERVICE_URL").unwrap_or_else(|_| "http://token-service:8082".into()),
    ));
    let state = AppState::new(
        db_pool,
        cache,
        config.clone(),
        redis,
        outbox_repo,
        jwks,
        service_jwt,
        token_forward,
    );

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    let mut app = routes::build_router(state);
    if swagger_enabled {
        // TODO(iam/swagger-segfault): 25 端点 + 1797 行 spec 触发 utoipa-swagger-ui v9 SIGSEGV 139（其他服务正常，疑似 schemas 循环引用）。Box 化未解决，临时禁 swagger。
        tracing::warn!("iam swagger ui DISABLED (SIGSEGV 139 in utoipa-swagger-ui merge); see TODO(iam/swagger-segfault)");
    } else {
        app = app
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
        info!("swagger ui enabled at /swagger-ui");
    }

    let addr: SocketAddr = format!("{}:{}", config.app.host, config.app.port).parse()?;
    info!("Starting iam-service on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        info!("received shutdown signal, deregistering from consul");
        let _ = jwks_shutdown.send(true);
        std::process::exit(0);
    });

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
