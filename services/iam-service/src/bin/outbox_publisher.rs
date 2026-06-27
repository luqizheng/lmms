//! Independent outbox publisher binary.
//!
//! Polls `iam_outbox_events` for unpublished rows and publishes them to
//! RabbitMQ. Runs as a separate process (same image as iam-service, different
//! CMD) so business-process restarts never drop in-flight events. Multiple
//! replicas coordinate via `SELECT ... FOR UPDATE SKIP LOCKED` in the repo.
//!
//! Resilience:
//! - **Broker bounce:** on publish/connect failure, sleeps and rebuilds the
//!   `RabbitMqPublisher` next cycle. Outbox rows stay unpublished until success
//!   — no data loss; spec §12 broker-restart acceptance criterion is met.
//! - **DB hiccup:** transient `claim_batch` / `mark_published` errors are
//!   logged + retried on the next cycle.
//! - **Graceful shutdown (SIGINT + SIGTERM):** stops claiming new batches but
//!   lets the current batch finish (publish + mark) before exit, so a
//!   published-but-unmarked row is never left behind by our own shutdown.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use common_event::{MySqlOutboxRepo, OutboxRepo, Publisher, RabbitMqPublisher};
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

    // Shutdown flag — flipped by SIGINT or SIGTERM. Checked between batches.
    let shutdown = Arc::new(AtomicBool::new(false));
    {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            warn!("shutdown signal received, finishing current batch then exiting");
            shutdown.store(true, Ordering::SeqCst);
        });
    }

    info!(poll_ms, batch_size, "outbox publisher ready, entering poll loop");

    let poll_interval = Duration::from_millis(poll_ms);
    let mut publisher: Option<RabbitMqPublisher> = None;

    while !shutdown.load(Ordering::SeqCst) {
        // Ensure the publisher is connected. On broker bounce, this rebuilds.
        if publisher.is_none() {
            match RabbitMqPublisher::new(&event_bus_url, &exchange).await {
                Ok(p) => {
                    info!("connected to RabbitMQ");
                    publisher = Some(p);
                }
                Err(e) => {
                    warn!(error = %e, "connect rabbitmq failed, will retry");
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }
            }
        }
        let pub_ref = publisher.as_ref().unwrap();

        // Claim a batch. DB hiccups: log + retry; outbox rows stay unpublished.
        let batch = match repo.claim_batch(batch_size).await {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "claim_batch failed, will retry");
                tokio::time::sleep(poll_interval).await;
                continue;
            }
        };
        if batch.is_empty() {
            tokio::time::sleep(poll_interval).await;
            continue;
        }

        // Publish each row. On broker failure, drop the publisher so the next
        // cycle reconnects. Rows that didn't get marked stay unpublished and
        // will be re-claimed (idempotent because consumer keys on event_id).
        let mut published_ids = Vec::with_capacity(batch.len());
        let mut broker_failed = false;
        for row in &batch {
            let body = match serde_json::to_vec(&row.envelope_json) {
                Ok(b) => b,
                Err(e) => {
                    warn!(error = %e, id = row.id, "skipping unserializable row");
                    continue;
                }
            };
            match pub_ref
                .publish_raw(&row.routing_key, &row.event_id.to_string(), &body)
                .await
            {
                Ok(()) => published_ids.push(row.id),
                Err(e) => {
                    warn!(error = %e, id = row.id, "publish failed, will reconnect");
                    broker_failed = true;
                    break;
                }
            }
        }

        if !published_ids.is_empty() {
            if let Err(e) = repo.mark_published(&published_ids).await {
                warn!(error = %e, "mark_published failed; rows will be reclaimed");
            } else {
                info!(published = published_ids.len(), "published outbox batch");
            }
        }

        if broker_failed {
            publisher = None; // force reconnect next cycle
            tokio::time::sleep(poll_interval).await;
        }
    }

    info!("outbox publisher exited cleanly");
    Ok(())
}
