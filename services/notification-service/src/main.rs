#![allow(dead_code)]

mod auth;
mod config;
mod consumer;
mod db;
mod error;
mod models;
mod recipient_resolver;
mod service;
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
use prometheus::{Encoder, TextEncoder};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config/config.yaml".to_string());
    let cfg = AppConfig::load(std::path::Path::new(&path))?;
    common_telemetry::init_tracing("notification-service");
    common_ratelimit::init_metrics("notification-service");
    // Tell rate-limit middleware which service it is (for metric labels)
    std::env::set_var("SERVICE_NAME", "notification-service");
    println!("==== notification-service config ====");
    println!("config_path: {}", path);
    match serde_json::to_string_pretty(&cfg) {
        Ok(s) => println!("{}", s),
        Err(e) => tracing::warn!("failed to serialize config: {}", e),
    }
    println!("======================================");

    let db = Db::connect(&cfg.mysql.url, cfg.mysql.max_connections).await?;
    db.ensure_schema().await?;
    tracing::info!("notification-service schema ensured");
    let redis = RedisCli::connect(&cfg.redis.url).await?;
    let hub = WsHub::new();

    let rest_state = rest::RestState {
        db: db.clone(),
        redis: redis.clone(),
        rate_limiter: common_ratelimit::RateLimiter::new(
            redis.conn.clone(),
            rest::default_limits(),
        ),
    };
    let ws_state = ws::WsState { hub: hub.clone() };

    async fn metrics() -> axum::response::Html<String> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        axum::response::Html(String::from_utf8(buffer).unwrap())
    }


    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    let mut app = Router::new()
        .nest("/notification", rest::router(rest_state))
        .route(
            "/notification/ws",
            get(ws::upgrade).with_state(ws_state),
        )
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .route("/metrics", get(metrics))
        .layer(axum::middleware::from_fn(
            notification_service::tracing_mw::trace_request_middleware,
        ));

    if swagger_enabled {
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", notification_service::openapi::ApiDoc::openapi()),
        );
        tracing::info!("swagger ui enabled at /swagger-ui");
    }

    let db_for_sched = db.clone();
    let redis_for_sched = redis.clone();
    let hub_for_sched = hub.clone();
    let tick = cfg.scheduler.tick_secs;
    tokio::spawn(async move {
        scheduler::run(db_for_sched, redis_for_sched, hub_for_sched, tick).await;
    });

    let db_for_consumer = db.clone();
    let redis_for_consumer = redis.clone();
    let hub_for_consumer = hub.clone();
    let event_bus_url = std::env::var("EVENT_BUS_URL").unwrap_or(cfg.event_bus.url.clone());
    tokio::spawn(async move {
        if let Err(e) = consumer::run(db_for_consumer.pool.clone(), redis_for_consumer, hub_for_consumer, event_bus_url).await {
            tracing::warn!("notification consumer exited: {}", e);
        }
    });

    let port: u16 = cfg
        .server
        .http_addr
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        &cfg.consul.service_id_file,
    );
    match consul_registrar
        .register_discovered(port, &cfg.consul.service_name)
        .await
    {
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

fn mask_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let after = &url[scheme_end + 3..];
    let Some(at) = after.find('@') else {
        return url.to_string();
    };
    let creds = &after[..at];
    let host_part = &after[at..];
    let Some(colon) = creds.find(':') else {
        return url.to_string();
    };
    let user = &creds[..colon];
    format!("{}://{}:***{}", &url[..scheme_end], user, host_part)
}

fn log_config(cfg: &AppConfig) {
    let mysql_url = mask_url(&cfg.mysql.url);
    let redis_url = mask_url(&cfg.redis.url);
    println!("========== notification-service config ==========");
    println!("server.http_addr          : {}", cfg.server.http_addr);
    println!("mysql.url                 : {}", mysql_url);
    println!("mysql.max_connections     : {}", cfg.mysql.max_connections);
    println!("redis.url                 : {}", redis_url);
    println!("scheduler.tick_secs       : {}", cfg.scheduler.tick_secs);
    println!("log.level                 : {}", cfg.log.level);
    println!("consul.addr               : {}", cfg.consul.addr);
    println!("consul.service_name       : {}", cfg.consul.service_name);
    println!("consul.service_id_file    : {}", cfg.consul.service_id_file);
    println!("================================================");
}
