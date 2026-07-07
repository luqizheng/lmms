use audit_service::{config::AppConfig, routes, create_state};
use common::db::connect;
use common_telemetry::init_tracing;
use std::net::SocketAddr;
use tracing::{info, warn};

const INIT_SQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const SCHEMA_MARKER_TABLE: &str = "audit_operation_logs";

async fn ensure_schema(pool: &sqlx::mysql::MySqlPool) -> anyhow::Result<()> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(SCHEMA_MARKER_TABLE)
    .fetch_one(pool)
    .await?;
    if row.0 > 0 {
        info!(marker = SCHEMA_MARKER_TABLE, "audit schema present, skip migration");
        return Ok(());
    }

    info!("audit schema missing, applying initial migration");
    for stmt in INIT_SQL.split(';') {
        let cleaned: String = stmt
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        let s = cleaned.trim();
        if s.is_empty() {
            continue;
        }
        let upper = s.to_ascii_uppercase();
        if upper.starts_with("CREATE DATABASE") || upper.starts_with("USE ") {
            continue;
        }
        if let Err(e) = sqlx::query(s).execute(pool).await {
            return Err(anyhow::anyhow!("apply audit migration: {e}"));
        }
    }
    info!("audit initial migration applied");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing("audit-service");
    audit_service::risk::init_metrics();

    let config = AppConfig::from_env();
    info!("Starting audit-service on {}", config.server_addr);

    let pool = connect(&config.database_url, 10)
        .await
        .expect("Failed to initialize database pool");

    if let Err(e) = ensure_schema(&pool).await {
        warn!("ensure_schema failed: {}", e);
    }

    // Start the RabbitMQ consumer that ingests iam.* events into audit_events.
    let event_bus_url = std::env::var("EVENT_BUS_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());
    {
        let pool = pool.clone();
        let event_bus_url = event_bus_url.clone();
        tokio::spawn(async move {
            if let Err(e) = audit_service::consumer::run(pool, event_bus_url).await {
                warn!("audit consumer failed to start: {}", e);
            }
        });
    }

    // 风控 consumer（独立队列 audit.login-risk）
    {
        let opts = audit_service::risk::RiskOptions {
            threshold: config.login_risk_threshold,
            window_secs: config.login_risk_window_secs,
        };
        let redis_url = config.redis_url.clone();
        tokio::spawn(async move {
            match audit_service::redis_cli::RedisCli::connect(&redis_url).await {
                Ok(redis) => {
                    if let Err(e) = audit_service::risk::run(redis, event_bus_url, opts).await {
                        warn!("risk consumer exited: {}", e);
                    }
                }
                Err(e) => warn!("risk consumer: redis connect failed: {}", e),
            }
        });
    }

    let state = create_state(pool);

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    info!("audit-service listening on {}", addr);

    let app = routes::routes(state);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false);
    let app = if swagger_enabled {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        info!("swagger ui enabled at /swagger-ui");
        app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", audit_service::openapi::ApiDoc::openapi()),
        )
    } else {
        app
    };

    
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        info!("received shutdown signal, deregistering from consul");
        std::process::exit(0);
    });

    axum::serve(listener, app).await.expect("Server error");
    Ok(())
}
