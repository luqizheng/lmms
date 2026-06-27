use std::time::Duration;

use tokio::time::sleep;

use crate::error::RunError;
use crate::outbox::OutboxRepo;
use crate::publisher::Publisher;

#[derive(Debug, Clone)]
pub struct OutboxPublisherOptions {
    pub poll_interval: Duration,
    pub batch_size: u32,
}

impl Default for OutboxPublisherOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(1000),
            batch_size: 100,
        }
    }
}

pub struct OutboxPublisher<R: OutboxRepo, P: Publisher> {
    repo: R,
    publisher: P,
    opts: OutboxPublisherOptions,
}

impl<R: OutboxRepo, P: Publisher> OutboxPublisher<R, P> {
    pub fn new(repo: R, publisher: P, opts: OutboxPublisherOptions) -> Self {
        Self {
            repo,
            publisher,
            opts,
        }
    }

    /// Run one poll cycle: claim a batch, publish each row, mark published.
    /// Returns the number of rows published this cycle. Exposed for tests and
    /// for callers that want to drive the loop themselves.
    pub async fn run_once(&self) -> Result<usize, RunError> {
        let batch = self
            .repo
            .claim_batch(self.opts.batch_size)
            .await
            .map_err(|e| RunError::Publish(e.to_string()))?;
        if batch.is_empty() {
            return Ok(0);
        }
        let mut published_ids = Vec::with_capacity(batch.len());
        for row in &batch {
            let body =
                serde_json::to_vec(&row.envelope_json).map_err(|e| RunError::Publish(e.to_string()))?;
            self.publisher
                .publish_raw(&row.routing_key, &row.event_id.to_string(), &body)
                .await
                .map_err(|e| RunError::Publish(e.to_string()))?;
            published_ids.push(row.id);
        }
        self.repo
            .mark_published(&published_ids)
            .await
            .map_err(|e| RunError::Publish(e.to_string()))?;
        Ok(published_ids.len())
    }

    /// Blocking run loop — **TEST HELPER ONLY**. Production publishers should
    /// own their own loop with reconnect-on-broker-failure semantics (see
    /// `services/iam-service/src/bin/outbox_publisher.rs` for the production
    /// pattern). This `run()` propagates errors via `?` and will exit on the
    /// first transient failure (DB unavailable on startup race, broker bounce,
    /// etc.) — fine for unit tests, fatal in production.
    pub async fn run(self) -> Result<(), RunError> {
        loop {
            let published = self.run_once().await?;
            if published == 0 {
                sleep(self.opts.poll_interval).await;
            }
        }
    }
}
