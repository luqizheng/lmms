//! Independent outbox publisher for notification-service (Phase 4.3).
//! Polls `ntf_outbox_events` and publishes to RabbitMQ. Same image as
//! notification-service, different CMD. Mirrors iam-service/src/bin/outbox_publisher.rs.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common_event::{MySqlOutboxRepo, OutboxRepo, Publisher, RabbitMqPublisher};
use sqlx::mysql::MySqlPoolOptions;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    common_telemetry::init_tracing("ntf-outbox-publisher");
    info!("ntf-outbox-publisher starting");

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let host = std::env::var("DATABASE_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port = std::env::var("DATABASE_PORT").unwrap_or_else(|_| "3306".into());
        let user = std::env::var("DATABASE_USER").unwrap_or_else(|_| "root".into());
        let pass = std::env::var("DATABASE_PASSWORD").unwrap_or_else(|_| "rootpass".into());
        let name = std::env::var("DATABASE_NAME").unwrap_or_else(|_| "superpowers".into());
        format!("mysql://{user}:{pass}@{host}:{port}/{name}")
    });
    let event_bus_url = std::env::var("EVENT_BUS_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());
    let exchange = std::env::var("EVENT_BUS_EXCHANGE").unwrap_or_else(|_| "events".to_string());
    let poll_ms: u64 = std::env::var("OUTBOX_POLL_INTERVAL_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(1000);
    let batch_size: u32 = std::env::var("OUTBOX_BATCH_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(100);

    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&db_url)
        .await?;
    let repo = MySqlOutboxRepo::with_table(pool, "ntf_outbox_events");

    let shutdown = Arc::new(AtomicBool::new(false));
    {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut term = signal(SignalKind::terminate()).expect("install SIGTERM");
                tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
            }
            #[cfg(not(unix))]
            { let _ = tokio::signal::ctrl_c().await; }
            warn!("shutdown signal received, finishing current batch then exiting");
            shutdown.store(true, Ordering::SeqCst);
        });
    }

    info!(poll_ms, batch_size, "ntf outbox publisher ready");
    let poll_interval = Duration::from_millis(poll_ms);
    let mut publisher: Option<RabbitMqPublisher> = None;

    while !shutdown.load(Ordering::SeqCst) {
        if publisher.is_none() {
            match RabbitMqPublisher::new(&event_bus_url, &exchange).await {
                Ok(p) => { info!("connected to RabbitMQ"); publisher = Some(p); }
                Err(e) => { warn!(error = %e, "connect rabbitmq failed, retry"); tokio::time::sleep(poll_interval).await; continue; }
            }
        }
        let pub_ref = publisher.as_ref().unwrap();
        let batch = match repo.claim_batch(batch_size).await {
            Ok(b) => b,
            Err(e) => { warn!(error = %e, "claim_batch failed, retry"); tokio::time::sleep(poll_interval).await; continue; }
        };
        if batch.is_empty() { tokio::time::sleep(poll_interval).await; continue; }

        let mut published_ids = Vec::with_capacity(batch.len());
        let mut broker_failed = false;
        for row in &batch {
            let body = match serde_json::to_vec(&row.envelope_json) {
                Ok(b) => b,
                Err(e) => { warn!(error = %e, id = row.id, "skip unserializable row"); continue; }
            };
            match pub_ref.publish_raw(&row.routing_key, &row.event_id.to_string(), &body).await {
                Ok(()) => published_ids.push(row.id),
                Err(e) => { warn!(error = %e, id = row.id, "publish failed, reconnect"); broker_failed = true; break; }
            }
        }
        if !published_ids.is_empty() {
            if let Err(e) = repo.mark_published(&published_ids).await {
                warn!(error = %e, "mark_published failed; rows will be reclaimed");
            } else {
                info!(published = published_ids.len(), "published ntf outbox batch");
            }
        }
        if broker_failed { publisher = None; tokio::time::sleep(poll_interval).await; }
    }
    info!("ntf outbox publisher exited cleanly");
    Ok(())
}
