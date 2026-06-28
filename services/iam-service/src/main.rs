use iam_service::{config::load_config, db, openapi::ApiDoc, routes, state::AppState};
use common_telemetry;
use common_cache::MemoryCache;
use consul_registrar::ConsulRegistrar;
use tracing::{info, warn};
use std::net::SocketAddr;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    common_telemetry::init_tracing("iam-service");

    let config = load_config()?;
    info!("Loaded config: {:?}", config.app.name);

    let db_pool = db::connect(&config.database).await?;
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

    let state = AppState::new(db_pool, cache, config.clone(), redis);

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
    
    let consul_addr = format!("http://{}:{}", config.consul.host, config.consul.port);
    let mut consul_registrar = ConsulRegistrar::new(
        &consul_addr,
        &config.consul.service_name,
        "/tmp/iam-service-id",
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;

    match consul_registrar
        .register_discovered(config.app.port, &config.consul.service_name)
        .await
    {
        Ok(_) => info!(
            "iam-service registered with consul (fallback {})",
            config.consul.service_name
        ),
        Err(e) => warn!("failed to register with consul: {}", e),
    }
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        info!("received shutdown signal, deregistering from consul");
        let _ = consul_registrar.deregister().await;
        std::process::exit(0);
    });
    
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    
    Ok(())
}