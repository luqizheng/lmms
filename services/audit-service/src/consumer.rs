//! RabbitMQ consumer: ingests iam.* domain events and stores them idempotently
//! in `audit_events`. Replaces the old synchronous HTTP write endpoints.
//!
//! Resilience: a top-level reconnect loop wraps the subscribe call so a
//! broker bounce (channel closed, connection drop) is handled by reconnecting
//! after a short backoff. Events that were unacked at the time of the drop
//! get redelivered by RabbitMQ on the new connection; the idempotency key
//! (event_id) on `audit_events` ensures we don't write duplicates.

use std::time::Duration;

use common_event::{Consumer, Envelope, HandlerError, RabbitMqConsumer};
use sqlx::mysql::MySqlPool;
use tracing::{info, warn};

/// Start the audit consumer. Reconnects on broker failure with a fixed 5s
/// backoff; returns only if a signal asks it to (i.e. essentially never —
/// the caller spawns it as a tokio task at startup).
pub async fn run(pool: MySqlPool, event_bus_url: String) -> anyhow::Result<()> {
    let backoff = Duration::from_secs(5);
    loop {
        match try_subscribe(pool.clone(), &event_bus_url).await {
            Ok(consumer) => {
                // Hold the consumer alive, polling connection health.
                // Re-subscribe ONLY on actual disconnect — durable queue is
                // idempotent so re-declare is safe, but a healthy broker
                // shouldn't pay the cost every 30s. See notification
                // announcement_run for the ephemeral counterpart.
                while consumer.connection().status().connected() {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                warn!("audit consumer connection lost, resubscribing");
            }
            Err(e) => {
                warn!(error = %e, "audit consumer subscribe failed, retry in 5s");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_subscribe(pool: MySqlPool, event_bus_url: &str) -> anyhow::Result<RabbitMqConsumer> {
    let consumer = RabbitMqConsumer::new(event_bus_url).await?;
    consumer
        .subscribe::<serde_json::Value, _, _>(
            "audit.iam-events",
            &["iam.#"],
            move |env: Envelope<serde_json::Value>| {
                let pool = pool.clone();
                async move { persist(&pool, &env).await }
            },
        )
        .await?;
    info!("audit consumer subscribed to audit.iam-events (iam.#)");
    Ok(consumer)
}

async fn persist(pool: &MySqlPool, env: &Envelope<serde_json::Value>) -> Result<(), HandlerError> {
    // env.payload is already a serde_json::Value — bind it directly.
    sqlx::query(
        "INSERT INTO audit_events \
         (event_id, event_type, occurred_at, trace_id, producer, payload_json) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON DUPLICATE KEY UPDATE received_at = CURRENT_TIMESTAMP(3)",
    )
    .bind(env.event_id.to_string())
    .bind(&env.event_type)
    .bind(env.occurred_at)
    .bind(&env.trace_id)
    .bind(&env.producer)
    .bind(&env.payload)
    .execute(pool)
    .await
    .map_err(|e| {
        warn!(error = %e, event_id = %env.event_id, "audit consumer persist failed");
        HandlerError::Handler(e.to_string())
    })?;
    info!(event_id = %env.event_id, event_type = %env.event_type, "audit event persisted");
    Ok(())
}
