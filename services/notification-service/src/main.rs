#![allow(dead_code)]

mod auth;
mod config;
mod consumer;
mod db;
mod error;
mod iam_client;
mod migration_state_route;
mod models;
mod recipient_resolver;
mod redis_cli;
mod rest;
mod scheduler;
mod service;
mod ws;
mod ws_hub;

use axum::routing::get;
use axum::Router;
use config::AppConfig;
use db::Db;
use digua_common_http::{NoopObserver, ResilienceConfig, ResilientClient};
use digua_common_jwt::{JWKSCache, JwksConfig, ServiceJwtCache, ServiceJwtConfig};
use iam_client::IamMemberClient;
use prometheus::{Encoder, TextEncoder};
use redis_cli::RedisCli;
use std::sync::Arc;
use std::time::Duration;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use ws_hub::WsHub;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    digua_common_telemetry::init_tracing("notification-service");
    digua_common_ratelimit::init_metrics("notification-service");
    // Tell rate-limit middleware which service it is (for metric labels)
    std::env::set_var("SERVICE_NAME", "notification-service");

    let cfg = AppConfig::load()?;
    println!("==== notification-service config ====");
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
    // when this service comes up; previously a single failed `refresh()` was
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
            caller_service_name: "notification-service".into(),
            caller_tenant: std::env::var("TENANT_ID")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1i64),
            refresh_skew: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        },
        http_for_jwt,
        String::new(),
    ));

    let resilient = Arc::new(ResilientClient::new(
        reqwest::Client::new(),
        ResilienceConfig::default(),
        Arc::new(NoopObserver),
    ));
    let iam_client = Arc::new(IamMemberClient::new(
        resilient,
        cfg.iam_base_url.clone(),
        service_jwt.clone(),
    ));

    // Phase 5B-fu3: bootstrap local user membership projection from iam.
    // Must happen BEFORE the membership consumer starts, so the consumer
    // doesn't see a half-empty projection on its first events.
    if let Err(e) = service::projection::bootstrap(&db, &iam_client).await {
        tracing::error!(error = %e, "membership bootstrap failed — projection empty; consumer will heal as events flow");
        // Fail-fast alternative: return Err(e). For now, log + continue
        // (consumer will eventually fill the projection via events; readiness
        // probe should reflect degraded state separately).
    } else {
        tracing::info!("membership bootstrap complete");
    }

    let rest_state = rest::RestState {
        db: db.clone(),
        redis: redis.clone(),
        rate_limiter: digua_common_ratelimit::RateLimiter::new(
            redis.conn.clone(),
            rest::default_limits(),
        ),
        iam_client: iam_client.clone(),
        jwks: jwks.clone(),
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
        .nest("/notification", rest::router(rest_state.clone()))
        .merge(migration_state_route::routes(rest_state.clone()).layer(
            axum::middleware::from_fn_with_state(
                rest_state.clone(),
                digua_authz::verify_aud_principal_middleware::<rest::RestState>(
                    "notification-service",
                ),
            ),
        ))
        .route("/notification/ws", get(ws::upgrade).with_state(ws_state))
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .route("/metrics", get(metrics))
        .layer(axum::middleware::from_fn(
            digua_common_telemetry_axum::trace_request_middleware,
        ));

    if swagger_enabled {
        app = app.merge(SwaggerUi::new("/swagger-ui").url(
            "/api-docs/openapi.json",
            notification_service::openapi::ApiDoc::openapi(),
        ));
        tracing::info!("swagger ui enabled at /swagger-ui");
    }

    let db_for_sched = db.clone();
    let tick = cfg.scheduler.tick_secs;
    tokio::spawn(async move {
        scheduler::run(db_for_sched, tick).await;
    });

    let db_for_consumer = db.clone();
    let redis_for_consumer = redis.clone();
    let outbox_repo_for_consumer = db.outbox_repo.clone();
    let event_bus_url = cfg.event_bus.url.clone();
    tokio::spawn(async move {
        if let Err(e) = consumer::run(
            db_for_consumer.pool.clone(),
            redis_for_consumer,
            outbox_repo_for_consumer,
            event_bus_url,
        )
        .await
        {
            tracing::warn!("notification consumer exited: {}", e);
        }
    });

    let hub_for_ann = hub.clone();
    let ann_event_bus_url = cfg.event_bus.url.clone();
    tokio::spawn(async move {
        if let Err(e) = consumer::announcement_run(hub_for_ann, ann_event_bus_url).await {
            tracing::warn!("announcement consumer exited: {}", e);
        }
    });

    // Phase 5B-fu3: membership event consumer (notification.membership-events)
    let db_for_membership = db.clone();
    let membership_event_bus_url = cfg.event_bus.url.clone();
    tokio::spawn(async move {
        if let Err(e) = consumer::membership::run(db_for_membership, membership_event_bus_url).await
        {
            tracing::warn!("membership consumer exited: {}", e);
        }
    });

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        tracing::info!("received shutdown signal");
        let _ = jwks_shutdown.send(true);
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
