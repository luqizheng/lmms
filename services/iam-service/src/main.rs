use iam_service::{config::load_config, db, routes, state::AppState};
use common_log::init_json;
use common_cache::MemoryCache;
use consul_registrar::ConsulRegistrar;
use tracing::{info, warn};
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
    
    let consul_addr = format!("http://{}:{}", config.consul.host, config.consul.port);
    let mut consul_registrar = ConsulRegistrar::new(
        &consul_addr,
        &config.consul.service_name,
        "/tmp/iam-service-id",
    );
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    let advertise_host = std::env::var("CONSUL_ADVERTISE_HOST")
        .unwrap_or_else(|_| config.consul.service_name.clone());
    match consul_registrar.register(&advertise_host, config.app.port).await {
        Ok(_) => info!("iam-service registered with consul as {}", advertise_host),
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