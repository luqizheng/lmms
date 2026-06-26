//! Health probes (live + ready) and graceful-shutdown shared state.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use once_cell::sync::Lazy;
use prometheus::{register_int_gauge_vec, IntGauge, IntGaugeVec};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

static SERVICE_READY: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "service_ready",
        "1 when /health/ready returns 200; 0 otherwise",
        &["service"]
    )
    .expect("service_ready gauge registration")
});

#[derive(Clone)]
enum DbProbe {
    Any(sqlx::AnyPool),
    MySql(sqlx::MySqlPool),
    Sqlite(sqlx::SqlitePool),
}

#[derive(Clone)]
pub struct HealthState {
    service_name: &'static str,
    db: Option<DbProbe>,
    redis: Option<redis::aio::ConnectionManager>,
    consul_addr: Option<String>,
    shutting_down: Arc<AtomicBool>,
    ready_gauge: IntGauge,
}

impl HealthState {
    pub fn new(service_name: &'static str) -> Self {
        let ready_gauge = SERVICE_READY.with_label_values(&[service_name]);
        ready_gauge.set(0);
        Self {
            service_name,
            db: None,
            redis: None,
            consul_addr: None,
            shutting_down: Arc::new(AtomicBool::new(false)),
            ready_gauge,
        }
    }

    pub fn with_db(mut self, pool: sqlx::AnyPool) -> Self {
        self.db = Some(DbProbe::Any(pool));
        self
    }

    pub fn with_mysql(mut self, pool: sqlx::MySqlPool) -> Self {
        self.db = Some(DbProbe::MySql(pool));
        self
    }

    pub fn with_sqlite(mut self, pool: sqlx::SqlitePool) -> Self {
        self.db = Some(DbProbe::Sqlite(pool));
        self
    }

    pub fn with_redis(mut self, conn: redis::aio::ConnectionManager) -> Self {
        self.redis = Some(conn);
        self
    }

    pub fn with_consul(mut self, addr: impl Into<String>) -> Self {
        self.consul_addr = Some(addr.into());
        self
    }

    pub fn mark_shutting_down(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        self.ready_gauge.set(0);
        tracing::info!(service = self.service_name, "marked NOT ready (shutting down)");
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    fn mark_ready(&self) {
        self.ready_gauge.set(1);
    }
}

pub fn health_routes(state: HealthState) -> Router {
    Router::new()
        .route("/health", get(legacy))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(state)
}

async fn legacy() -> &'static str {
    "OK"
}

async fn live() -> &'static str {
    "OK"
}

async fn ready(State(s): State<HealthState>) -> impl IntoResponse {
    if s.is_shutting_down() {
        s.ready_gauge.set(0);
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "shutting_down"})),
        );
    }

    let mut report = serde_json::Map::new();
    let mut ok = true;

    if let Some(probe) = &s.db {
        let db_ok = match probe {
            DbProbe::Any(pool) => tokio::time::timeout(
                Duration::from_secs(1),
                sqlx::query("SELECT 1").execute(pool),
            )
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false),
            DbProbe::MySql(pool) => tokio::time::timeout(
                Duration::from_secs(1),
                sqlx::query("SELECT 1").execute(pool),
            )
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false),
            DbProbe::Sqlite(pool) => tokio::time::timeout(
                Duration::from_secs(1),
                sqlx::query("SELECT 1").execute(pool),
            )
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false),
        };
        if db_ok {
            report.insert("db".into(), "ok".into());
        } else {
            report.insert("db".into(), "fail".into());
            ok = false;
        }
    }

    if let Some(conn) = &s.redis {
        let mut conn = conn.clone();
        let pong: Result<Result<String, _>, _> = tokio::time::timeout(
            Duration::from_millis(500),
            redis::cmd("PING").query_async(&mut conn),
        )
        .await;
        match pong {
            Ok(Ok(_)) => {
                report.insert("redis".into(), "ok".into());
            }
            _ => {
                report.insert("redis".into(), "fail".into());
                ok = false;
            }
        }
    }

    if let Some(addr) = &s.consul_addr {
        let host_port = addr
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let host_port = host_port.split('/').next().unwrap_or(host_port);
        match tokio::time::timeout(
            Duration::from_millis(200),
            tokio::net::TcpStream::connect(host_port),
        )
        .await
        {
            Ok(Ok(_)) => {
                report.insert("consul".into(), "ok".into());
            }
            _ => {
                report.insert("consul".into(), "fail".into());
                ok = false;
            }
        }
    }

    if ok {
        s.mark_ready();
        (StatusCode::OK, Json(serde_json::Value::Object(report)))
    } else {
        s.ready_gauge.set(0);
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::Value::Object(report)),
        )
    }
}

/// SIGTERM/SIGINT 来时：mark shutting_down → drain → 返回
pub async fn shutdown_signal(state: HealthState) {
    let sigterm = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            if let Ok(mut s) = signal(SignalKind::terminate()) {
                s.recv().await;
            } else {
                std::future::pending::<()>().await;
            }
        }
        #[cfg(not(unix))]
        {
            std::future::pending::<()>().await;
        }
    };
    let sigint = tokio::signal::ctrl_c();
    tokio::select! {
        _ = sigterm => tracing::info!("SIGTERM received"),
        _ = sigint => tracing::info!("SIGINT received"),
    }
    state.mark_shutting_down();
    let drain_secs: u64 = std::env::var("SHUTDOWN_DRAIN_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    tracing::info!(secs = drain_secs, "draining before close");
    tokio::time::sleep(Duration::from_secs(drain_secs)).await;
}
