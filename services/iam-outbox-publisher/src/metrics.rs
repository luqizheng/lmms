//! Prometheus text-format metrics written to a file (no HTTP).
//!
//! `iam-outbox-publisher` is a CLI binary, not a server: it has no
//! `/metrics` endpoint (see the service AGENTS.md 豁免 — the process
//! exporter covers the basics). For richer SLI, we expose the minimum
//! useful set as a Prometheus text file and write it atomically on every
//! poll cycle so a sidecar / node-exporter `textfile` collector can
//! scrape it from `/var/run/prometheus/...`.
//!
//! Metric catalogue:
//!   - `outbox_poll_lag_seconds` (gauge, f64)   — seconds between consecutive poll starts
//!   - `outbox_batch_size` (gauge, int)         — rows claimed in the most recent batch
//!   - `outbox_published_total` (counter)       — rows successfully published + marked
//!   - `outbox_error_total{error_type}` (counter) — errors grouped by failure point
//!
//! `flush_to_file` is best-effort: failures log a warning and return; the
//! publisher loop keeps running. The write is atomic (write to a sibling
//! `.tmp`, then `rename` into place) so a concurrent reader — Prometheus
//! textfile scrape — never sees a torn file. Multi-replica safety is the
//! deployment's responsibility: each pod must write to its own path
//! (e.g. an emptyDir per pod), since the on-disk values are per-process.

use std::path::Path;
use std::sync::OnceLock;

use prometheus::{Encoder, Gauge, IntCounter, IntCounterVec, IntGauge, TextEncoder};
use tracing::warn;

/// All metric names live here so they show up in one search and so the
/// round-trip test below can reference them by symbol.
const POLL_LAG: &str = "outbox_poll_lag_seconds";
const BATCH_SIZE: &str = "outbox_batch_size";
const PUBLISHED_TOTAL: &str = "outbox_published_total";
const ERROR_TOTAL: &str = "outbox_error_total";

struct Metrics {
    poll_lag: Gauge,
    batch_size: IntGauge,
    published_total: IntCounter,
    error_total: IntCounterVec,
}

/// Lazy global registry wrapper. All `register_*!` calls hit the prometheus
/// default registry, which `prometheus::gather()` reads back.
fn metrics() -> &'static Metrics {
    static M: OnceLock<Metrics> = OnceLock::new();
    M.get_or_init(|| {
        Metrics {
            poll_lag: prometheus::register_gauge!(
                POLL_LAG,
                "Seconds elapsed between the previous poll cycle's start and the current cycle's start (poll-lag SLI)"
            )
            .expect("register outbox_poll_lag_seconds"),
            batch_size: prometheus::register_int_gauge!(
                BATCH_SIZE,
                "Number of outbox rows claimed in the most recent batch"
            )
            .expect("register outbox_batch_size"),
            published_total: prometheus::register_int_counter!(
                PUBLISHED_TOTAL,
                "Total outbox rows successfully published and marked as published"
            )
            .expect("register outbox_published_total"),
            error_total: prometheus::register_int_counter_vec!(
                ERROR_TOTAL,
                "Total outbox publisher errors, grouped by failure point (error_type label)",
                &["error_type"]
            )
            .expect("register outbox_error_total"),
        }
    })
}

/// Update `outbox_poll_lag_seconds`. Pass `0.0` on the very first cycle.
pub fn record_poll_lag(seconds: f64) {
    metrics().poll_lag.set(seconds);
}

/// Update `outbox_batch_size`. `0` means an empty / idle poll cycle.
pub fn record_batch_size(size: usize) {
    metrics().batch_size.set(size as i64);
}

/// Increment `outbox_published_total` by `n`. No-op when `n == 0`.
pub fn record_published(n: usize) {
    if n > 0 {
        metrics().published_total.inc_by(n as u64);
    }
}

/// Increment `outbox_error_total{error_type=<error_type>}`.
pub fn record_error(error_type: &str) {
    metrics().error_total.with_label_values(&[error_type]).inc();
}

/// Encode the current registry snapshot to Prometheus text format and
/// write it to `path` atomically. Returns silently on failure (with a
/// warn log); never panics, never propagates I/O errors to the caller's
/// poll loop.
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

    // Atomic write: dump to a sibling `.tmp`, then `rename` into place.
    // POSIX `rename` within the same directory is atomic, so a concurrent
    // reader (Prometheus textfile scrape) never sees a half-written file.
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
