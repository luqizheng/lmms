//! Independent outbox publisher for notification-service (Phase 4.3).
//! Polls `ntf_outbox_events` and publishes to RabbitMQ. Same image as
//! notification-service, different CMD. Mirrors iam-service/src/bin/outbox_publisher.rs.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use digua_common_event::{MySqlOutboxRepo, OutboxRepo, Publisher, RabbitMqPublisher};
use sqlx::mysql::MySqlPoolOptions;
use tracing::{info, warn};

mod metrics {
    //! Prometheus text-format metrics written to a file (no HTTP).
    //!
    //! Same shape as `iam-outbox-publisher`'s metrics module — the two
    //! binaries are siblings and want a consistent SLI surface. Kept
    //! inline because the `notification-service` outbox publisher is a
    //! single-file binary (no `src/metrics.rs` to share with a sibling
    //! module).
    //!
    //! See `services/iam-outbox-publisher/src/metrics.rs` for the
    //! long-form rationale.
    use std::path::Path;
    use std::sync::OnceLock;

    use prometheus::{Encoder, Gauge, IntCounter, IntCounterVec, IntGauge, TextEncoder};
    use tracing::warn;

    struct Metrics {
        poll_lag: Gauge,
        batch_size: IntGauge,
        published_total: IntCounter,
        error_total: IntCounterVec,
    }

    fn metrics() -> &'static Metrics {
        static M: OnceLock<Metrics> = OnceLock::new();
        M.get_or_init(|| {
            Metrics {
                poll_lag: prometheus::register_gauge!(
                    "outbox_poll_lag_seconds",
                    "Seconds elapsed between the previous poll cycle's start and the current cycle's start (poll-lag SLI)"
                )
                .expect("register outbox_poll_lag_seconds"),
                batch_size: prometheus::register_int_gauge!(
                    "outbox_batch_size",
                    "Number of outbox rows claimed in the most recent batch"
                )
                .expect("register outbox_batch_size"),
                published_total: prometheus::register_int_counter!(
                    "outbox_published_total",
                    "Total outbox rows successfully published and marked as published"
                )
                .expect("register outbox_published_total"),
                error_total: prometheus::register_int_counter_vec!(
                    "outbox_error_total",
                    "Total outbox publisher errors, grouped by failure point (error_type label)",
                    &["error_type"]
                )
                .expect("register outbox_error_total"),
            }
        })
    }

    pub fn record_poll_lag(seconds: f64) {
        metrics().poll_lag.set(seconds);
    }

    pub fn record_batch_size(size: usize) {
        metrics().batch_size.set(size as i64);
    }

    pub fn record_published(n: usize) {
        if n > 0 {
            metrics().published_total.inc_by(n as u64);
        }
    }

    pub fn record_error(error_type: &str) {
        metrics()
            .error_total
            .with_label_values(&[error_type])
            .inc();
    }

    /// Encode the current registry snapshot to Prometheus text format and
    /// write it to `path` atomically. Returns silently on failure (with a
    /// warn log); never panics.
    pub fn flush_to_file(path: &Path) {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    warn!(path = %path.display(), error = %e, "create metrics dir failed");
                    return;
                }
            }
        }

        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buf = Vec::with_capacity(2048);
        if let Err(e) = encoder.encode(&metric_families, &mut buf) {
            warn!(error = %e, "encode prometheus text failed");
            return;
        }

        let mut tmp_os = path.as_os_str().to_owned();
        tmp_os.push(".tmp");
        let tmp_path = Path::new(&tmp_os);
        if let Err(e) = std::fs::write(tmp_path, &buf) {
            warn!(path = %tmp_path.display(), error = %e, "write metrics tmp failed");
            return;
        }
        if let Err(e) = std::fs::rename(tmp_path, path) {
            warn!(
                from = %tmp_path.display(),
                to = %path.display(),
                error = %e,
                "rename metrics into place failed"
            );
            return;
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    digua_common_telemetry::init_tracing("ntf-outbox-publisher");
    info!("ntf-outbox-publisher starting");

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        // ponytail: dev convenience only — compose + AGENTS.md require DATABASE_URL.
        // ponytail: 移除旧的 DATABASE_HOST/PORT/USER/PASSWORD/NAME fallback,统一用 DATABASE_URL
        "mysql://root:rootpass@127.0.0.1:3306/superpowers".to_string()
    });
    let event_bus_url = std::env::var("EVENT_BUS_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());
    let exchange = std::env::var("EVENT_BUS_EXCHANGE").unwrap_or_else(|_| "events".to_string());
    let poll_ms: u64 = std::env::var("OUTBOX_POLL_INTERVAL_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(1000);
    let batch_size: u32 = std::env::var("OUTBOX_BATCH_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(100);
    let metrics_path = std::env::var("METRICS_FILE_PATH")
        .unwrap_or_else(|_| "/var/run/prometheus/ntf_outbox.prom".to_string());
    let metrics_path_buf: PathBuf = Path::new(&metrics_path).to_path_buf();

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
    let mut last_poll_start: Option<Instant> = None;

    while !shutdown.load(Ordering::SeqCst) {
        let poll_start = Instant::now();
        let lag_seconds = last_poll_start
            .map(|prev| poll_start.duration_since(prev).as_secs_f64())
            .unwrap_or(0.0);
        metrics::record_poll_lag(lag_seconds);
        last_poll_start = Some(poll_start);

        if publisher.is_none() {
            match RabbitMqPublisher::new(&event_bus_url, &exchange).await {
                Ok(p) => { info!("connected to RabbitMQ"); publisher = Some(p); }
                Err(e) => {
                    warn!(error = %e, "connect rabbitmq failed, retry");
                    metrics::record_error("broker_connect");
                    tokio::time::sleep(poll_interval).await;
                    metrics::flush_to_file(&metrics_path_buf);
                    continue;
                }
            }
        }
        let pub_ref = publisher.as_ref().unwrap();
        let batch = match repo.claim_batch(batch_size).await {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "claim_batch failed, retry");
                metrics::record_error("claim_batch");
                tokio::time::sleep(poll_interval).await;
                metrics::flush_to_file(&metrics_path_buf);
                continue;
            }
        };
        metrics::record_batch_size(batch.len());
        if batch.is_empty() {
            tokio::time::sleep(poll_interval).await;
            metrics::flush_to_file(&metrics_path_buf);
            continue;
        }

        let mut published_ids = Vec::with_capacity(batch.len());
        let mut broker_failed = false;
        for row in &batch {
            let body = match serde_json::to_vec(&row.envelope_json) {
                Ok(b) => b,
                Err(e) => {
                    warn!(error = %e, id = row.id, "skip unserializable row");
                    metrics::record_error("serialize");
                    continue;
                }
            };
            match pub_ref.publish_raw(&row.routing_key, &row.event_id.to_string(), &body).await {
                Ok(()) => published_ids.push(row.id),
                Err(e) => {
                    warn!(error = %e, id = row.id, "publish failed, reconnect");
                    metrics::record_error("publish");
                    broker_failed = true;
                    break;
                }
            }
        }
        if !published_ids.is_empty() {
            if let Err(e) = repo.mark_published(&published_ids).await {
                warn!(error = %e, "mark_published failed; rows will be reclaimed");
                metrics::record_error("mark_published");
            } else {
                info!(published = published_ids.len(), "published ntf outbox batch");
                metrics::record_published(published_ids.len());
            }
        }
        if broker_failed {
            publisher = None;
            tokio::time::sleep(poll_interval).await;
        }
        metrics::flush_to_file(&metrics_path_buf);
    }
    metrics::flush_to_file(&metrics_path_buf);
    info!("ntf outbox publisher exited cleanly");
    Ok(())
}
