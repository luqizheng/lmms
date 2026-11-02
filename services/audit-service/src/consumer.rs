//! RabbitMQ consumer: ingests iam.* and pmd.* domain events and stores them
//! idempotently in `audit_events`. Replaces the old synchronous HTTP write
//! endpoints.
//!
//! Two subscriptions, two top-level reconnect loops, ONE persist handler:
//! - iam routing-key subspace → queue `audit.iam-events`, routing_keys `iam.#`
//! - pmd routing-key subspace → queue `audit.pmd-events`, routing_keys `pmd.#`
//!
//! Adding a new subdomain = new queue + new `run_*` + new spawn in main.rs;
//! `persist()` is unchanged.
//!
//! Resilience: each top-level reconnect loop wraps its subscribe call so a
//! broker bounce (channel closed, connection drop) is handled by reconnecting
//! after a short backoff. Events that were unacked at the time of the drop
//! get redelivered by RabbitMQ on the new connection; the idempotency key
//! (event_id) on `audit_events` ensures we don't write duplicates.
//!
//! Dialect notes:
//! - 幂等写入语法两侧不同：MySQL `ON DUPLICATE KEY UPDATE` vs PostgreSQL
//!   `ON CONFLICT (event_id) DO UPDATE`（依赖唯一索引 `uk_audit_events_event_id`）。
//! - `payload_json` 是 JSON 列：PG 侧写入必须显式 cast `?::json`，否则 sqlx 以
//!   JSONB 发送会撞 `42804 column is of type json but expression is of type jsonb`。
//!   读回时对应地要写 `payload_json::text`（见 `tests/pg_roundtrip_test.rs`）。

use std::time::Duration;

use digua_common_db::dialect::DialectKind;
use digua_common_db::{db_execute, DbPool};
use digua_common_event::{
    span_from_envelope_trace_id, Consumer, Envelope, HandlerError, RabbitMqConsumer,
};
use tracing::{info, warn, Instrument};

/// MySQL 幂等写入（依赖 `uk_audit_events_event_id`）。
const INSERT_EVENT_SQL_MYSQL: &str = "INSERT INTO audit_events \
     (event_id, event_type, occurred_at, trace_id, producer, payload_json) \
     VALUES (?, ?, ?, ?, ?, ?) \
     ON DUPLICATE KEY UPDATE received_at = CURRENT_TIMESTAMP(3)";

/// PostgreSQL 幂等写入（`ON CONFLICT (event_id)` + json 列显式 cast）。
const INSERT_EVENT_SQL_POSTGRES: &str = "INSERT INTO audit_events \
     (event_id, event_type, occurred_at, trace_id, producer, payload_json) \
     VALUES (?, ?, ?, ?, ?, ?::json) \
     ON CONFLICT (event_id) DO UPDATE SET received_at = CURRENT_TIMESTAMP(3)";

/// Start the audit consumer for the iam.* subspace (`audit.iam-events` queue).
///
/// Reconnects on broker failure with a fixed 5s backoff; returns only if a
/// signal asks it to (i.e. essentially never — the caller spawns it as a
/// tokio task at startup).
pub async fn run(pool: DbPool, event_bus_url: String) -> anyhow::Result<()> {
    subscribe_loop(pool, event_bus_url, "audit.iam-events", &["iam.#"]).await
}

/// Phase 9 addition: pmd.* subspace consumer. Same persist handler as iam —
/// the queue + binding separation is purely for ops isolation (per-queue DLQ
/// counts, per-queue consumer counts in Prometheus). The on-disk row shape is
/// identical (audit_events.event_type = the routing-key suffix).
pub async fn run_pmd(pool: DbPool, event_bus_url: String) -> anyhow::Result<()> {
    subscribe_loop(pool, event_bus_url, "audit.pmd-events", &["pmd.#"]).await
}

async fn subscribe_loop(
    pool: DbPool,
    event_bus_url: String,
    queue_name: &'static str,
    routing_keys: &'static [&'static str],
) -> anyhow::Result<()> {
    let backoff = Duration::from_secs(5);
    loop {
        match try_subscribe(&pool, &event_bus_url, queue_name, routing_keys).await {
            Ok(consumer) => {
                // Hold the consumer alive, polling connection health.
                // Re-subscribe ONLY on actual disconnect — durable queue is
                // idempotent so re-declare is safe, but a healthy broker
                // shouldn't pay the cost every 30s. See notification
                // announcement_run for the ephemeral counterpart.
                while consumer.connection().status().connected() {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                warn!(
                    queue = queue_name,
                    "audit consumer connection lost, resubscribing"
                );
            }
            Err(e) => {
                warn!(error = %e, queue = queue_name, "audit consumer subscribe failed, retry in 5s");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_subscribe(
    pool: &DbPool,
    event_bus_url: &str,
    queue_name: &'static str,
    routing_keys: &'static [&'static str],
) -> anyhow::Result<RabbitMqConsumer> {
    let consumer = RabbitMqConsumer::new(event_bus_url).await?;
    let pool_clone = pool.clone();
    consumer
        .subscribe::<serde_json::Value, _, _>(
            queue_name,
            routing_keys,
            move |env: Envelope<serde_json::Value>| {
                // Build span synchronously: `span_from_envelope_trace_id` requires
                // `&'static str`, which borrowing inside the async future can't provide.
                let span = span_from_envelope_trace_id(&env, queue_name);
                let pool = pool_clone.clone();
                async move { persist(&pool, &env).instrument(span).await }
            },
        )
        .await?;
    info!(
        queue = queue_name,
        ?routing_keys,
        "audit consumer subscribed"
    );
    Ok(consumer)
}

/// 幂等落库一条事件。
///
/// `pub` 的原因：audit-service 没有 HTTP 写入入口（事件全部来自 MQ），集成测试
/// 要验证 PG 上的 `ON CONFLICT` 与 `json` 列往返只能直接调这里——手抄一份
/// INSERT SQL 的测试在生产 SQL 改动时不会红，等于自证。
pub async fn persist(
    pool: &DbPool,
    env: &Envelope<serde_json::Value>,
) -> Result<(), HandlerError> {
    let sql = match pool.dialect() {
        DialectKind::Postgres => INSERT_EVENT_SQL_POSTGRES,
        _ => INSERT_EVENT_SQL_MYSQL,
    };

    // env.payload is already a serde_json::Value — bind it directly
    // （PG 分支的 `?::json` 由置位符改写器转成 `$6::json`）。
    db_execute!(
        pool,
        sql,
        env.event_id.to_string(),
        &env.event_type,
        env.occurred_at,
        &env.trace_id,
        &env.producer,
        &env.payload
    )
    .map_err(|e| {
        warn!(error = %e, event_id = %env.event_id, "audit consumer persist failed");
        // 简化：所有 sqlx 错默认 Transient（DB 抖动多见），业务层 unique-violation
        // 留给 ON DUPLICATE KEY / ON CONFLICT UPDATE 吸收；未来可按
        // sqlx::Error::is_unique_violation() 拆 Permanent
        HandlerError::Transient(e.to_string())
    })?;
    info!(event_id = %env.event_id, event_type = %env.event_type, "audit event persisted");
    Ok(())
}
