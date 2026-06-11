#![allow(dead_code)]

mod auth;
mod config;
mod db;
mod error;
mod models;
mod recipient_resolver;
mod redis_cli;
mod rest;
mod scheduler;
mod ws;
mod ws_hub;

use axum::routing::get;
use axum::Router;
use config::AppConfig;
use consul_registrar::ConsulRegistrar;
use db::Db;
use redis_cli::RedisCli;
use ws_hub::WsHub;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config/config.yaml".to_string());
    let cfg = AppConfig::load(std::path::Path::new(&path))?;
    tracing_subscriber::fmt::init();

    let db = Db::connect(&cfg.mysql.url, cfg.mysql.max_connections).await?;
    let redis = RedisCli::connect(&cfg.redis.url).await?;
    let hub = WsHub::new();

    let rest_state = rest::RestState {
        db: db.clone(),
        redis: redis.clone(),
    };
    let ws_state = ws::WsState { hub: hub.clone() };

    let app = Router::new()
        .nest("/api/notifications", rest::router(rest_state))
        .route(
            "/api/notifications/ws",
            get(ws::upgrade).with_state(ws_state),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        );

    let db_for_sched = db.clone();
    let redis_for_sched = redis.clone();
    let hub_for_sched = hub.clone();
    let tick = cfg.scheduler.tick_secs;
    tokio::spawn(async move {
        scheduler::run(db_for_sched, redis_for_sched, hub_for_sched, tick).await;
    });

    let host = cfg.server.http_addr.split(':').next().unwrap_or("0.0.0.0").to_string();
    let port: u16 = cfg
        .server
        .http_addr
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8083);

    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        &cfg.consul.service_id_file,
    );
    match consul_registrar.register(&host, port).await {
        Ok(_) => tracing::info!("notification-service registered with consul"),
        Err(e) => tracing::warn!("failed to register with consul: {}", e),
    }
    let consul_for_shutdown = consul_registrar;
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        tracing::info!("received shutdown signal, deregistering from consul");
        let _ = consul_for_shutdown.deregister().await;
        std::process::exit(0);
    });

    let listener = tokio::net::TcpListener::bind(&cfg.server.http_addr).await?;
    tracing::info!("notification-service listening on {}", cfg.server.http_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
