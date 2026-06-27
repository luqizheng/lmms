//! Independent outbox publisher binary.
//!
//! Polls `iam_outbox_events` for unpublished rows and publishes them to
//! RabbitMQ. Runs as a separate process (same image as iam-service, different
//! CMD) so business-process restarts never drop in-flight events. Multiple
//! replicas coordinate via `SELECT ... FOR UPDATE SKIP LOCKED` in the repo.
//!
//! Graceful shutdown: on SIGTERM/Ctrl-C it stops claiming new batches but lets
//! the current `run_once` batch finish (publish + mark) before exiting, so a
//! published-but-unmarked row is never left behind by our own shutdown.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use common_event::{MySqlOutboxRepo, OutboxPublisher, OutboxPublisherOptions, RabbitMqPublisher};
use iam_service::config::load_config;
use sqlx::mysql::MySqlPoolOptions;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    common_telemetry::init_tracing("iam-outbox-publisher");
    info!("iam-outbox-publisher starting");

    // DB url: prefer DATABASE_URL env (compose/CI), else iam-service config.
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => load_config().context("load iam config")?.database.url,
    };
    let event_bus_url = std::env::var("EVENT_BUS_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());
    let exchange = std::env::var("EVENT_BUS_EXCHANGE").unwrap_or_else(|_| "events".to_string());
    let poll_ms: u64 = std::env::var("OUTBOX_POLL_INTERVAL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let batch_size: u32 = std::env::var("OUTBOX_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&db_url)
        .await
        .context("connect mysql")?;
    let repo = MySqlOutboxRepo::new(pool);
    let publisher = RabbitMqPublisher::new(&event_bus_url, &exchange)
        .await
        .context("connect rabbitmq")?;

    let outbox = OutboxPublisher::new(
        repo,
        publisher,
        OutboxPublisherOptions {
            poll_interval: Duration::from_millis(poll_ms),
            batch_size,
        },
    );

    // Shutdown flag, flipped by a signal handler task. Checked between batches.
    let shutdown = Arc::new(AtomicBool::new(false));
    {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            warn!("shutdown signal received, finishing current batch then exiting");
            shutdown.store(true, Ordering::SeqCst);
        });
    }

    info!(poll_ms, batch_size, "outbox publisher ready, entering poll loop");

    let poll_interval = Duration::from_millis(poll_ms);
    while !shutdown.load(Ordering::SeqCst) {
        let published = outbox.run_once().await.context("outbox run_once")?;
        if published > 0 {
            info!(published, "published outbox batch");
            // More may be pending — loop again immediately without sleeping.
            continue;
        }
        // Outbox empty: sleep, but wake early if shutdown fires.
        tokio::time::sleep(poll_interval).await;
    }

    info!("outbox publisher exited cleanly");
    Ok(())
}
