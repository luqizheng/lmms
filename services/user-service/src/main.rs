use anyhow::Result;
use user_service::config::load_config;
use user_service::db::init_pool;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = load_config("config/config.yaml")?;
    let pool = init_pool(&cfg.database).await?;
    tracing::info!(db_type = ?cfg.database.db_type, "user-service-rust pool ready");

    tracing::info!("user-service-rust started on {}:{}", cfg.server.host, cfg.server.port);
    let _ = pool; // placeholder

    Ok(())
}
