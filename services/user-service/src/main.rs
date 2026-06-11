use anyhow::Result;
use consul_registrar::ConsulRegistrar;
use tower_http::cors::CorsLayer;
use user_service::config::load_config;
use user_service::db::init_pool;
use user_service::routes::auth::{router as auth_router, AuthState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = load_config("config/config.yaml")?;
    let pool = init_pool(&cfg.database).await?;
    let redis_client = redis::Client::open(cfg.redis.url.clone())?;

    tracing::info!(db_type = ?cfg.database.db_type, "user-service-rust pool ready");

    let state = AuthState {
        pool,
        redis: redis_client,
        cfg: cfg.clone(),
    };
    let app = auth_router(state)
        .route(
            "/health",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .layer(CorsLayer::permissive());

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        &cfg.consul.service_id_file,
    );
    match consul_registrar
        .register(&cfg.server.host, cfg.server.port)
        .await
    {
        Ok(_) => tracing::info!("user-service registered with consul"),
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

    tracing::info!("user-service-rust listening on {addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
