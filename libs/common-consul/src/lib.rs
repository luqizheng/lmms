//! Consul self-registration for platform services.
//!
//! Replaces the retired `gliderlabs/registrator` sidecar (2026-11-05): each
//! service process registers itself with the local Consul agent at startup,
//! reconciles every [`RECONCILE_INTERVAL`] in case the catalog lost the entry
//! (Consul data dir wiped, agent restarted empty), and deregisters on
//! shutdown (SIGTERM handler awaits [`ConsulGuard::deregister`]; the guard's
//! `Drop` also fires a best-effort deregister task).
//!
//! # Env contract (set in `infrastructure/docker-compose-services.yml`)
//!
//! | Variable | Default | Meaning |
//! |---|---|---|
//! | `CONSUL_ADDR` | `http://consul:8500` | Consul agent HTTP endpoint |
//! | `SERVICE_NAME` | *(none)* | Catalog service name; **unset ⇒ registration disabled** (bare host runs) |
//! | `SERVICE_PORT` | *(none)* | Port the service listens on (catalog `port` + health check URL) |
//! | `SERVICE_ADDRESS` | hostname | Address Consul checks will dial (container name on `superpowers`) |
//! | `SERVICE_HEALTH_PATH` | `/health` | HTTP health check path |
//! | `SERVICE_TAGS` | `backend,<name>` | Comma-separated catalog tags |
//!
//! # Usage
//!
//! ```no_run
//! #[tokio::main]
//! async fn main() {
//!     let guard = digua_common_consul::consul_guard();
//!     // ... serve until SIGTERM ...
//!     guard.deregister().await; // then exit
//! }
//! ```

use once_cell::sync::Lazy;
use prometheus::{register_int_gauge, IntGauge};
use serde::Serialize;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{info, warn};

/// Initial registration retry cadence while Consul is unreachable (e.g. cold
/// start race where the service boots before the Consul healthcheck passes).
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
/// Periodic reconciliation: re-register when the agent no longer knows us.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
/// Hard ceiling on how long a health check may stay critical before Consul
/// drops the registration (covers SIGKILL / power-loss, where no graceful
/// deregister can run).
const DEREGISTER_CRITICAL_AFTER: &str = "2m";
/// Best-effort cap on the synchronous deregister performed on shutdown.
const DEREGISTER_TIMEOUT: Duration = Duration::from_secs(2);

static REGISTRATION_UP: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "consul_registration_up",
        "1 when this instance is registered in Consul"
    )
    .expect("consul_registration_up gauge registration")
});

// ── Env contract ────────────────────────────────────────────────────────────

/// Everything needed to register one service instance, resolved from env.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationConfig {
    /// Consul agent HTTP endpoint, e.g. `http://consul:8500`.
    pub consul_addr: String,
    /// Catalog service name (e.g. `iam-service`).
    pub service_name: String,
    /// Unique-per-instance catalog id (`<name>-<hostname>`).
    pub service_id: String,
    /// Address Consul checks dial (container name on the shared network).
    pub address: String,
    /// Port the service listens on.
    pub port: u16,
    /// Health check path (e.g. `/health`).
    pub health_path: String,
    /// Catalog tags (e.g. `["backend", "iam-service"]`).
    pub tags: Vec<String>,
}

impl RegistrationConfig {
    /// Read the env contract. Returns `Ok(None)` when registration is
    /// intentionally disabled (`SERVICE_NAME` unset — bare host runs, tests).
    pub fn from_env() -> Result<Option<Self>, String> {
        let service_name = match std::env::var("SERVICE_NAME") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => return Ok(None),
        };
        let port: u16 = std::env::var("SERVICE_PORT")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .ok_or_else(|| {
                "SERVICE_PORT unset or not a u16 (required when SERVICE_NAME is set)".to_string()
            })?;
        let address = match std::env::var("SERVICE_ADDRESS") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => hostname_or_uuid(),
        };
        let tags = match std::env::var("SERVICE_TAGS") {
            Ok(v) if !v.trim().is_empty() => v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            _ => vec!["backend".to_string(), service_name.clone()],
        };
        let service_id = format!("{}-{}", service_name, hostname_or_uuid_for_id());
        Ok(Some(Self {
            consul_addr: std::env::var("CONSUL_ADDR")
                .unwrap_or_else(|_| "http://consul:8500".into()),
            service_name,
            service_id,
            address,
            port,
            health_path: std::env::var("SERVICE_HEALTH_PATH").unwrap_or_else(|_| "/health".into()),
            tags,
        }))
    }
}

/// Container hostname (compose sets it to the container name on the shared
/// network) — the address Consul's health check will dial.
fn hostname_or_uuid() -> String {
    hostname().unwrap_or_else(|| format!("instance-{}", uuid::Uuid::new_v4()))
}

/// Hostnames are unique per container, so `{name}-{hostname}` is a stable,
/// unique service id. Fall back to a random suffix when hostname is empty.
fn hostname_or_uuid_for_id() -> String {
    match hostname() {
        Some(h) if !h.trim().is_empty() => h,
        _ => uuid::Uuid::new_v4().simple().to_string(),
    }
}

fn hostname() -> Option<String> {
    // std::env::var is the portable subset here: /etc/hostname is what the
    // HOSTNAME env var mirrors in every container we ship.
    std::env::var("HOSTNAME")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

// ── Consul payload ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CheckDefinition {
    http: String,
    interval: &'static str,
    timeout: &'static str,
    #[serde(rename = "deregister_critical_service_after")]
    deregister_critical_service_after: &'static str,
}

#[derive(Serialize)]
struct ServiceRegistration<'a> {
    name: &'a str,
    id: &'a str,
    address: &'a str,
    port: u16,
    tags: &'a [String],
    checks: Vec<CheckDefinition>,
}

impl RegistrationConfig {
    fn payload(&self) -> ServiceRegistration<'_> {
        ServiceRegistration {
            name: &self.service_name,
            id: &self.service_id,
            address: &self.address,
            port: self.port,
            tags: &self.tags,
            checks: vec![CheckDefinition {
                http: format!("http://{}:{}{}", self.address, self.port, self.health_path),
                interval: "15s",
                timeout: "3s",
                deregister_critical_service_after: DEREGISTER_CRITICAL_AFTER,
            }],
        }
    }
}

// ── Registration client ─────────────────────────────────────────────────────

/// Fresh-connection client: registration traffic is one request every
/// [`RECONCILE_INTERVAL`], so connection-pool reuse buys nothing — and reusing
/// an idle connection that the server has just closed surfaces as spurious
/// "error sending request" failures (observed with hyper/reqwest 0.12 against
/// wiremock on Windows). Keep-alive off keeps production and tests
/// deterministic.
fn make_client() -> reqwest::Client {
    reqwest::Client::builder()
        .pool_max_idle_per_host(0)
        .build()
        .expect("reqwest client")
}

pub async fn register(client: &reqwest::Client, cfg: &RegistrationConfig) -> Result<(), String> {
    let url = format!("{}/v1/agent/service/register", cfg.consul_addr);
    let resp = client
        .put(&url)
        .json(&cfg.payload())
        .send()
        .await
        .map_err(|e| format!("PUT register: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("PUT register: HTTP {}", resp.status()))
    }
}

pub async fn deregister(client: &reqwest::Client, cfg: &RegistrationConfig) -> Result<(), String> {
    let url = format!(
        "{}/v1/agent/service/deregister/{}",
        cfg.consul_addr, cfg.service_id
    );
    let resp = client
        .put(&url)
        .send()
        .await
        .map_err(|e| format!("PUT deregister: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("PUT deregister: HTTP {}", resp.status()))
    }
}

/// `Ok(true)` when the agent still holds our registration, `Ok(false)` when
/// it must be re-registered, `Err` on transport failure (retry next tick).
pub async fn is_registered(
    client: &reqwest::Client,
    cfg: &RegistrationConfig,
) -> Result<bool, String> {
    let url = format!(
        "{}/v1/agent/service/name/{}",
        cfg.consul_addr, cfg.service_id
    );
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => Ok(true),
        Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => Ok(false),
        Ok(resp) => Err(format!("GET service/name: HTTP {}", resp.status())),
        Err(e) => Err(format!("GET service/name: {e}")),
    }
}

// ── Low-level HTTP operations (exposed for tests & ops tooling) ────────────

// ── Guard ───────────────────────────────────────────────────────────────────

/// Owns this instance's registration lifecycle. Cheap to hold; drops cleanly
/// when registration is disabled.
pub struct ConsulGuard {
    shutdown_tx: watch::Sender<bool>,
    cfg: Option<RegistrationConfig>,
}

impl ConsulGuard {
    /// Disabled guard (no `SERVICE_NAME`): no-op on deregister.
    fn disabled() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self {
            shutdown_tx: tx,
            cfg: None,
        }
    }

    /// True when this guard actually manages a registration.
    pub fn enabled(&self) -> bool {
        self.cfg.is_some()
    }

    /// Graceful deregister (SIGTERM path). Bounded by [`DEREGISTER_TIMEOUT`];
    /// safe to call more than once.
    pub async fn deregister(&self) {
        let Some(cfg) = &self.cfg else { return };
        let _ = self.shutdown_tx.send(true);
        REGISTRATION_UP.set(0);
        let client = make_client();
        match tokio::time::timeout(DEREGISTER_TIMEOUT, deregister(&client, cfg)).await {
            Ok(Ok(())) => {
                info!(service = %cfg.service_name, id = %cfg.service_id, "deregistered from consul")
            }
            Ok(Err(e)) => warn!(service = %cfg.service_name, id = %cfg.service_id, "{e}"),
            Err(_) => {
                warn!(service = %cfg.service_name, id = %cfg.service_id, "deregister timed out")
            }
        }
    }
}

impl Drop for ConsulGuard {
    fn drop(&mut self) {
        if self.cfg.is_some() {
            let _ = self.shutdown_tx.send(true);
            // Best-effort: fire the deregister if we're inside a live runtime
            // (guard dropped before the SIGTERM handler ran). If the runtime
            // is already gone the spawn fails silently — acceptable, the 2m
            // deregister_critical_service_after sweeps dead instances.
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let cfg = self.cfg.clone().expect("checked is_some");
                handle.spawn(async move {
                    let client = make_client();
                    deregister(&client, &cfg).await.ok();
                });
            }
            REGISTRATION_UP.set(0);
        }
    }
}

/// Wait for SIGINT (all platforms) and SIGTERM (unix).
///
/// Services previously listened on `ctrl_c` only — `docker stop` sends
/// SIGTERM, which no handler consumed, so the process died on SIGKILL after
/// the stop grace period and never ran any graceful path. Centralising the
/// signal pair here lets every service's shutdown handler start with one
/// await before deregistering.
pub async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let term = signal(SignalKind::terminate());
        match term {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                // signal registration failed — degrade to ctrl-c only
                tokio::signal::ctrl_c().await.ok();
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.ok();
    }
}

/// Read the env contract and spawn the registration/reconciliation task.
///
/// Returns immediately; the first registration happens on the background
/// task (with retry while Consul is cold). Call [`ConsulGuard::deregister`]
/// from the SIGTERM handler before exiting.
pub fn consul_guard() -> ConsulGuard {
    let cfg = match RegistrationConfig::from_env() {
        Ok(Some(cfg)) => cfg,
        Ok(None) => {
            info!("consul registration disabled (SERVICE_NAME unset)");
            return ConsulGuard::disabled();
        }
        Err(e) => {
            // Misconfiguration must not take the service down (health checks
            // still gate traffic); log loudly and continue unregistered.
            warn!("consul registration disabled: {e}");
            return ConsulGuard::disabled();
        }
    };

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(run(cfg.clone(), shutdown_rx));
    ConsulGuard {
        shutdown_tx,
        cfg: Some(cfg),
    }
}

async fn run(cfg: RegistrationConfig, mut shutdown: watch::Receiver<bool>) {
    let client = make_client();

    // Initial registration with retry — services may boot before the Consul
    // healthcheck turns green.
    loop {
        if *shutdown.borrow() {
            return;
        }
        match register(&client, &cfg).await {
            Ok(()) => {
                REGISTRATION_UP.set(1);
                info!(service = %cfg.service_name, id = %cfg.service_id, address = %cfg.address, port = cfg.port, "registered in consul");
                break;
            }
            Err(e) => {
                warn!(service = %cfg.service_name, "{e}; retrying in {:?}", RETRY_INTERVAL);
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
        }
    }

    // Reconcile loop: repair a lost catalog entry (agent restarted empty,
    // data dir wiped). Exit promptly on shutdown signal or guard drop.
    loop {
        tokio::select! {
            _ = tokio::time::sleep(RECONCILE_INTERVAL) => {}
            _ = shutdown.changed() => {
                deregister(&client, &cfg).await.ok();
                return;
            }
        }
        match is_registered(&client, &cfg).await {
            Ok(true) => {}
            Ok(false) => match register(&client, &cfg).await {
                Ok(()) => REGISTRATION_UP.set(1),
                Err(e) => {
                    REGISTRATION_UP.set(0);
                    warn!(service = %cfg.service_name, "re-register failed: {e}");
                }
            },
            Err(e) => warn!(service = %cfg.service_name, "reconcile probe failed: {e}"),
        }
    }
}
