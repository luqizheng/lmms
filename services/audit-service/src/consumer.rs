//! RabbitMQ consumer: ingests iam.* domain events and stores them idempotently
//! in `audit_events`. Replaces the old synchronous HTTP write endpoints.

use common_event::{Consumer, Envelope, HandlerError, RabbitMqConsumer};
use sqlx::mysql::MySqlPool;
use tracing::{info, warn};

/// Start the audit consumer. Subscribes the durable `audit.iam-events` queue
/// (bound to `iam.#`) and persists each event into `audit_events`.
pub async fn run(pool: MySqlPool, event_bus_url: &str) -> anyhow::Result<()> {
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
    Ok(())
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
