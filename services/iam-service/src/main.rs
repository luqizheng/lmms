use iam_service::{config::load_config, db, routes, state::AppState};
use common_log::setup_logging;
use consul_registrar::ConsulRegistrar;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging();
    
    let config = load_config()?;
    info!("Loaded config: {:?}", config.app.name);
    
    let db_pool = db::init_pool(&config.database).await?;
    let redis_client = common_cache::new_redis_client(&config.redis).await?;
    
    let state = AppState::new(db_pool, redis_client, config.clone());
    
    let app = routes::build_router(state);
    
    let addr = format!("{}:{}", config.app.host, config.app.port).parse()?;
    info!("Starting iam-service on {}", addr);
    
    #[cfg(not(test))]
    {
        let _registrar = ConsulRegistrar::new(
            &config.consul,
            "iam-service",
            addr.to_string(),
        ).await?;
    }
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}
