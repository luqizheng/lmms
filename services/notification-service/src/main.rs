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
        );

    let db_for_sched = db.clone();
    let redis_for_sched = redis.clone();
    let hub_for_sched = hub.clone();
    let tick = cfg.scheduler.tick_secs;
    tokio::spawn(async move {
        scheduler::run(db_for_sched, redis_for_sched, hub_for_sched, tick).await;
    });

    let listener = tokio::net::TcpListener::bind(&cfg.server.http_addr).await?;
    tracing::info!("notification-service listening on {}", cfg.server.http_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
