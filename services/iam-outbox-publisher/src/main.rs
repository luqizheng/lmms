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
//!
//! Config: mirrors iam-service's three-tier semantics for `database.url`:
//!     env DATABASE_URL > CONFIG_PATH yaml > (fail fast).
//! The embedded default yaml (same file iam-service uses) is seeded to
//! `CONFIG_PATH` on first boot if absent; `create_new` handles the race with
//! iam-service starting in parallel. Empty/blank `database.url` after the
//! merge → hard error at startup.

mod metrics;

use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use digua_common_db::DbPool;
use digua_common_event::{outbox_archive, MySqlOutboxRepo, OutboxRepo, Publisher, RabbitMqPublisher};
use sqlx::mysql::MySqlPoolOptions;
use tracing::{info, warn};

/// Shared with iam-service; compile-time embedded. Both processes seed this
/// same content, and `create_new` in `seed_if_absent` guarantees exactly one
/// writer wins the race.
const DEFAULT_YAML: &str = include_str!("../../iam-service/config/config.default.yaml");

const DEFAULT_CONFIG_PATH: &str = "/etc/iam-service/config.yaml";

/// Load `database.url` with the same priority ladder as iam-service:
///     env DATABASE_URL > CONFIG_PATH yaml > (fail fast).
/// Any resolved value that is empty / whitespace-only is treated as unset.
fn load_db_url() -> anyhow::Result<String> {
    if let Ok(u) = std::env::var("DATABASE_URL") {
        let trimmed = u.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
        warn!("DATABASE_URL is set but blank; falling back to yaml");
    }

    let config_path =
        std::env::var("CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
    let path = Path::new(&config_path);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match seed_if_absent(path) {
            Ok(true) => info!(path = %config_path, "seeded config.yaml from embedded default"),
            Ok(false) => {} // peer (iam-service) beat us to it, fine
            Err(e) => warn!(
                error = %e,
                path = %config_path,
                "seed failed; will fall back to embedded default yaml input"
            ),
        }
    }

    let yaml_content = std::fs::read_to_string(path).unwrap_or_else(|e| {
        warn!(
            error = %e,
            path = %config_path,
            "config file unreadable; using embedded default as yaml input"
        );
        DEFAULT_YAML.to_string()
    });

    #[derive(serde::Deserialize)]
    struct DbConfig {
        database: DatabaseSection,
    }
    #[derive(serde::Deserialize)]
    struct DatabaseSection {
        #[serde(default)]
        url: String,
    }
    let cfg: DbConfig = serde_yaml::from_str(&yaml_content).context("parse iam config yaml")?;
    let url = cfg.database.url.trim();
    if url.is_empty() {
        return Err(anyhow!(
            "database.url unset — provide DATABASE_URL env or set database.url in {config_path}"
        ));
    }
    Ok(url.to_string())
}

fn seed_if_absent(path: &Path) -> std::io::Result<bool> {
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => {
            f.write_all(DEFAULT_YAML.as_bytes())?;
            Ok(true)
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    digua_common_telemetry::init_tracing("iam-outbox-publisher");
    info!("iam-outbox-publisher starting");

    let db_url = load_db_url().context("resolve DATABASE_URL")?;
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
    let metrics_path = std::env::var("METRICS_FILE_PATH")
        .unwrap_or_else(|_| "/var/run/prometheus/iam_outbox.prom".to_string());

    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&db_url)
        .await
        .context("connect mysql")?;
    let repo = MySqlOutboxRepo::new(pool.clone());

    // Phase 6-A T4: archive cron runs as a sibling tokio task. Uses a clone
    // of the publisher's pool; multi-replica safety is handled by the dialect
    // lock (GET_LOCK / pg_try_advisory_lock) inside the shared outbox_archive.
    {
        let archive_pool = DbPool::from(pool.clone());
        tokio::spawn(async move {
            outbox_archive::run(archive_pool).await;
        });
        info!("outbox archive cron spawned");
    }

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

    info!(
        poll_ms,
        batch_size, "outbox publisher ready, entering poll loop"
    );

    let poll_interval = Duration::from_millis(poll_ms);
    let mut publisher: Option<RabbitMqPublisher> = None;
    let metrics_path_buf = Path::new(&metrics_path).to_path_buf();
    let mut last_poll_start: Option<Instant> = None;

    while !shutdown.load(Ordering::SeqCst) {
        let poll_start = Instant::now();
        let lag_seconds = last_poll_start
            .map(|prev| poll_start.duration_since(prev).as_secs_f64())
            .unwrap_or(0.0);
        metrics::record_poll_lag(lag_seconds);
        last_poll_start = Some(poll_start);

        // Ensure the publisher is connected. On broker bounce, this rebuilds.
        if publisher.is_none() {
            match RabbitMqPublisher::new(&event_bus_url, &exchange).await {
                Ok(p) => {
                    info!("connected to RabbitMQ");
                    publisher = Some(p);
                }
                Err(e) => {
                    warn!(error = %e, "connect rabbitmq failed, will retry");
                    metrics::record_error("broker_connect");
                    tokio::time::sleep(poll_interval).await;
                    metrics::flush_to_file(&metrics_path_buf);
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
                    metrics::record_error("serialize");
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
                info!(published = published_ids.len(), "published outbox batch");
                metrics::record_published(published_ids.len());
            }
        }

        if broker_failed {
            publisher = None; // force reconnect next cycle
            tokio::time::sleep(poll_interval).await;
        }
        metrics::flush_to_file(&metrics_path_buf);
    }

    metrics::flush_to_file(&metrics_path_buf);
    info!("outbox publisher exited cleanly");
    Ok(())
}
