use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{MySql, MySqlPool, Transaction};
use uuid::Uuid;

use crate::envelope::Envelope;
use crate::error::OutboxError;
use crate::event::Event;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxRow {
    pub id: i64,
    pub event_id: Uuid,
    pub routing_key: String,
    pub envelope_json: serde_json::Value,
}

#[async_trait]
pub trait OutboxRepo: Send + Sync {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &Envelope<P>,
    ) -> Result<(), OutboxError>;

    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError>;

    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError>;
}

pub struct MySqlOutboxRepo {
    pool: MySqlPool,
}

impl MySqlOutboxRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxRepo for MySqlOutboxRepo {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &Envelope<P>,
    ) -> Result<(), OutboxError> {
        let json = serde_json::to_value(env)?;
        sqlx::query(
            "INSERT INTO outbox_events (event_id, routing_key, envelope_json) \
             VALUES (?, ?, ?) \
             ON DUPLICATE KEY UPDATE id = id",
        )
        .bind(env.event_id.to_string())
        .bind(P::event_type())
        .bind(json)
        .execute(&mut **tx)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(())
    }

    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError> {
        // FOR UPDATE SKIP LOCKED: multiple publisher replicas grab disjoint rows
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;
        let rows: Vec<(i64, String, String, serde_json::Value)> = sqlx::query_as(
            "SELECT id, event_id, routing_key, envelope_json \
             FROM outbox_events \
             WHERE published_at IS NULL \
             ORDER BY id ASC \
             LIMIT ? \
             FOR UPDATE SKIP LOCKED",
        )
        .bind(limit as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;

        let out: Result<Vec<OutboxRow>, OutboxError> = rows
            .into_iter()
            .map(|(id, eid, rk, env)| {
                Ok(OutboxRow {
                    id,
                    event_id: Uuid::parse_str(&eid)
                        .map_err(|e| OutboxError::Database(e.to_string()))?,
                    routing_key: rk,
                    envelope_json: env,
                })
            })
            .collect();
        // Phase 4: at-least-once delivery. FOR UPDATE SKIP LOCKED releases the
        // lock on commit here, so the claim is advisory only — mark_published is
        // the real re-claim guard, backed by the consumer's idempotency key.
        // A later phase may span claim→publish→mark in one tx or add a lease column.
        tx.commit()
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;
        out
    }

    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "UPDATE outbox_events SET published_at = CURRENT_TIMESTAMP(3) WHERE id IN ({})",
            placeholders
        );
        let mut q = sqlx::query(&sql);
        for id in ids {
            q = q.bind(*id);
        }
        q.execute(&self.pool)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(())
    }
}
