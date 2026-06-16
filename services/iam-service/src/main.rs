use iam_service::{config::load_config, db, routes, state::AppState};
use common_log::init_json;
use common_cache::MemoryCache;
use tracing::info;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_json("info");
    
    let config = load_config()?;
    info!("Loaded config: {:?}", config.app.name);
    
    let db_pool = db::connect(&config.database).await?;
    let cache = MemoryCache::new(10000);
    
    let state = AppState::new(db_pool, cache, config.clone());
    
    let app = routes::build_router(state);
    
    let addr: SocketAddr = format!("{}:{}", config.app.host, config.app.port).parse()?;
    info!("Starting iam-service on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    
    Ok(())
}