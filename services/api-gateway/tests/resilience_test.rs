// 验证 ResilientClient 返回值与 gateway 映射的对应关系（决策 5A/12A）。
// Ok(resp) -> 透传; CircuitOpen->503; Timeout->504; Connect->502 (映射见 router.rs)。
use bytes::Bytes;
use common_http::{NoopObserver, ResilienceConfig, ResilientClient, ResilientError};
use reqwest::{header::HeaderMap, Client, Method, StatusCode};
use std::sync::Arc;
use std::time::Duration;

async fn spawn(app: axum::Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    format!("http://{addr}")
}

fn fast_cfg() -> ResilienceConfig {
    ResilienceConfig {
        timeout_ms: 200,
        overall_timeout_ms: 600,
        retry_max_attempts: 2,
        retry_base_delay_ms: 5,
        retry_max_delay_ms: 20,
        retry_jitter: 0.0,
        breaker_failure_threshold: 100,
        breaker_open_timeout_ms: 1000,
        breaker_half_open_max_calls: 1,
    }
}

fn rc() -> ResilientClient {
    ResilientClient::new(Client::new(), fast_cfg(), Arc::new(NoopObserver))
}

#[tokio::test]
async fn retry_exhausted_5xx_passes_through_body() {
    let app = axum::Router::new()
        .route("/x", axum::routing::get(|| async { (StatusCode::SERVICE_UNAVAILABLE, "down") }));
    let base = spawn(app).await;
    let resp = rc()
        .call("svc", Method::GET, &format!("{base}/x"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("ok upstream");
    assert_eq!(resp.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(&resp.body[..], b"down");
}

#[tokio::test]
async fn connect_error_maps_to_502_class() {
    let err = rc()
        .call("svc", Method::GET, "http://127.0.0.1:1/none", &HeaderMap::new(), Bytes::new())
        .await
        .expect_err("connect err");
    assert!(matches!(err, ResilientError::Connect(_)));
}

#[tokio::test]
async fn timeout_maps_to_504_class() {
    let app = axum::Router::new().route(
        "/slow",
        axum::routing::get(|| async {
            tokio::time::sleep(Duration::from_millis(400)).await;
            "late"
        }),
    );
    let base = spawn(app).await;
    let err = rc()
        .call("svc", Method::GET, &format!("{base}/slow"), &HeaderMap::new(), Bytes::new())
        .await
        .expect_err("timeout");
    assert!(matches!(err, ResilientError::Timeout));
}
