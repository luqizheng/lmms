use bytes::Bytes;
use common_http::CircuitState;
use common_http::{
    NoopObserver, Outcome, ResilienceConfig, ResilienceObserver, ResilientClient, ResilientError,
};
use reqwest::{header::HeaderMap, Client, Method, StatusCode};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

async fn spawn_server(app: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn cfg() -> ResilienceConfig {
    ResilienceConfig {
        timeout_ms: 300,
        overall_timeout_ms: 1000,
        retry_max_attempts: 3,
        retry_base_delay_ms: 10,
        retry_max_delay_ms: 50,
        retry_jitter: 0.0,
        breaker_failure_threshold: 3,
        breaker_open_timeout_ms: 200,
        breaker_half_open_max_calls: 2,
    }
}

fn client(cfg: ResilienceConfig, obs: Arc<dyn ResilienceObserver>) -> ResilientClient {
    ResilientClient::new(Client::new(), cfg, obs)
}

#[derive(Default)]
struct Recorder {
    outcomes: Mutex<Vec<Outcome>>,
    states: Mutex<Vec<CircuitState>>,
    retries: AtomicU64,
}
impl ResilienceObserver for Recorder {
    fn on_state_change(&self, _s: &str, st: CircuitState) {
        self.states.lock().unwrap().push(st);
    }
    fn on_outcome(&self, _s: &str, o: Outcome) {
        self.outcomes.lock().unwrap().push(o);
    }
    fn on_retry(&self, _s: &str) {
        self.retries.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn success_returns_owned_body_and_records_success() {
    let app = axum::Router::new().route("/ok", axum::routing::get(|| async { "hello" }));
    let base = spawn_server(app).await;
    let rec = Arc::new(Recorder::default());
    let c = client(cfg(), rec.clone());
    let resp = c
        .call("svc", Method::GET, &format!("{base}/ok"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("ok");
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(&resp.body[..], b"hello");
    assert_eq!(rec.outcomes.lock().unwrap()[0], Outcome::Success);
}

#[tokio::test]
async fn retries_502_for_idempotent_then_exhausts() {
    let hits = Arc::new(AtomicU64::new(0));
    let h2 = hits.clone();
    let app = axum::Router::new().route(
        "/bad",
        axum::routing::get(move || {
            let h = h2.clone();
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                StatusCode::BAD_GATEWAY
            }
        }),
    );
    let base = spawn_server(app).await;
    let rec = Arc::new(Recorder::default());
    let c = client(cfg(), rec.clone());
    let resp = c
        .call("svc", Method::GET, &format!("{base}/bad"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("ok upstream resp");
    assert_eq!(resp.status, StatusCode::BAD_GATEWAY); // 重试耗尽后透传 502（5A）
    assert_eq!(hits.load(Ordering::SeqCst), 3); // max_attempts 次
    assert!(rec.retries.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn post_502_not_retried() {
    let hits = Arc::new(AtomicU64::new(0));
    let h2 = hits.clone();
    let app = axum::Router::new().route(
        "/bad",
        axum::routing::post(move || {
            let h = h2.clone();
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                StatusCode::BAD_GATEWAY
            }
        }),
    );
    let base = spawn_server(app).await;
    let c = client(cfg(), Arc::new(NoopObserver));
    let _ = c
        .call("svc", Method::POST, &format!("{base}/bad"), &HeaderMap::new(), Bytes::new())
        .await;
    assert_eq!(hits.load(Ordering::SeqCst), 1); // 非幂等不重试
}

#[tokio::test]
async fn overall_deadline_caps_retries() {
    // 每次都慢于单次 timeout(300ms) → 触发超时重试，但 overall(1000ms) 封顶
    let app = axum::Router::new().route(
        "/slow",
        axum::routing::get(|| async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            "late"
        }),
    );
    let base = spawn_server(app).await;
    let rec = Arc::new(Recorder::default());
    let c = client(cfg(), rec.clone());
    let start = Instant::now();
    let err = c
        .call("svc", Method::GET, &format!("{base}/slow"), &HeaderMap::new(), Bytes::new())
        .await
        .expect_err("should time out");
    let elapsed = start.elapsed();
    assert!(matches!(err, ResilientError::Timeout));
    assert!(elapsed < Duration::from_millis(1500), "elapsed={elapsed:?}"); // 受 overall 约束
}

#[tokio::test]
async fn breaker_opens_then_short_circuits() {
    let app = axum::Router::new()
        .route("/bad", axum::routing::get(|| async { StatusCode::SERVICE_UNAVAILABLE }));
    let base = spawn_server(app).await;
    let rec = Arc::new(Recorder::default());
    let mut c = cfg();
    c.retry_max_attempts = 1; // 不重试，便于快速累积失败计数
    let client = client(c, rec.clone());
    for _ in 0..3 {
        let _ = client
            .call("svc", Method::GET, &format!("{base}/bad"), &HeaderMap::new(), Bytes::new())
            .await;
    }
    let err = client
        .call("svc", Method::GET, &format!("{base}/bad"), &HeaderMap::new(), Bytes::new())
        .await
        .expect_err("circuit open");
    assert!(matches!(err, ResilientError::CircuitOpen));
    assert!(rec.outcomes.lock().unwrap().contains(&Outcome::CircuitOpen));
    assert!(rec.states.lock().unwrap().contains(&CircuitState::Open));
}

#[tokio::test]
async fn breaker_recovers_after_open_timeout() {
    let fail = Arc::new(AtomicBool::new(true));
    let f2 = fail.clone();
    let app = axum::Router::new().route(
        "/flap",
        axum::routing::get(move || {
            let f = f2.clone();
            async move {
                if f.load(Ordering::SeqCst) {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::OK
                }
            }
        }),
    );
    let base = spawn_server(app).await;
    let mut c = cfg();
    c.retry_max_attempts = 1;
    let client = client(c, Arc::new(NoopObserver));
    for _ in 0..3 {
        let _ = client
            .call("svc", Method::GET, &format!("{base}/flap"), &HeaderMap::new(), Bytes::new())
            .await;
    }
    fail.store(false, Ordering::SeqCst); // 下游恢复
    tokio::time::sleep(Duration::from_millis(250)).await; // 过 open_timeout(200ms)
    let r1 = client
        .call("svc", Method::GET, &format!("{base}/flap"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("halfopen probe");
    assert_eq!(r1.status, StatusCode::OK);
    let r2 = client
        .call("svc", Method::GET, &format!("{base}/flap"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("recovered");
    assert_eq!(r2.status, StatusCode::OK);
}