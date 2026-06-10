use anyhow::Result;
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
    let app = auth_router(state).layer(CorsLayer::permissive());

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("user-service-rust listening on {addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
