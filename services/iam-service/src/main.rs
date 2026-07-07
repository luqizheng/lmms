use iam_service::{config::load_config, db, openapi::ApiDoc, routes, state::AppState};
use common_telemetry;
use common_cache::MemoryCache;
use common_event::MySqlOutboxRepo;
use tracing::{info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    common_telemetry::init_tracing("iam-service");

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

    let state = AppState::new(db_pool, cache, config.clone(), redis, outbox_repo);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    let mut app = routes::build_router(state);
    if swagger_enabled {
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
        );
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
        std::process::exit(0);
    });
    
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    
    Ok(())
}