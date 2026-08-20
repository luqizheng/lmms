use digua_common_jwt::{JWKSCache, JwksConfig};
use digua_common_telemetry::init_tracing;
use file_service::{
    config::{AppConfig, StorageBackend},
    db::{ensure_schema, init_pool},
    routes::{self, create_state},
    storage::{LocalStorage, S3Storage},
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("file-service");

    let config = AppConfig::from_env()?;
    let backend = config.backend()?;
    tracing::info!(
        "file-service starting on {} with backend={}",
        config.server_addr,
        backend.as_str()
    );

    if matches!(backend, StorageBackend::Local) {
        std::fs::create_dir_all(&config.storage_path).expect("failed to create storage directory");
    }

    let pool = init_pool(&config.database_url)
        .await
        .expect("failed to initialize database pool");

    ensure_schema(&pool).await.expect("ensure_schema failed");

    let storage: Arc<dyn file_service::storage::object_store::ObjectStore> = match backend {
        StorageBackend::Local => {
            tracing::info!("using LocalStorage at {}", config.storage_path);
            Arc::new(LocalStorage::new(&config.storage_path))
        }
        StorageBackend::Rustfs | StorageBackend::S3 => {
            let s3_cfg = config.s3_config()?;
            tracing::info!(
                "using S3Storage backend={} bucket={} endpoint={:?} path_style={} auto_create={}",
                backend.as_str(),
                s3_cfg.bucket_name,
                s3_cfg.endpoint,
                s3_cfg.path_style,
                s3_cfg.auto_create_bucket
            );
            Arc::new(S3Storage::new(s3_cfg)?)
        }
    };

    let base_url = "http://localhost:8082/file/".to_string();

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

    let state = create_state(pool, storage, base_url, config.max_file_size, jwks);

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
            file_service::openapi::ApiDoc::openapi(),
        ))
    } else {
        app
    };

    let addr: SocketAddr = config.server_addr.parse().expect("invalid server address");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    tracing::info!("file-service listening on {}", addr);

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        tracing::info!("received shutdown signal");
        let _ = jwks_shutdown.send(true);
        std::process::exit(0);
    });

    axum::serve(listener, app).await.expect("server error");
    Ok(())
}
