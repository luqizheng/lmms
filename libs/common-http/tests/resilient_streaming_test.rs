use bytes::Bytes;
use digua_common_http::{NoopObserver, ResilienceConfig, ResilientClient};
use reqwest::{header::HeaderMap, Client, Method};
use std::sync::Arc;
use std::time::Duration;

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
        timeout_ms: 1000,
        overall_timeout_ms: 2000,
        retry_max_attempts: 2,
        retry_base_delay_ms: 5,
        retry_max_delay_ms: 20,
        retry_jitter: 0.0,
        breaker_failure_threshold: 3,
        breaker_open_timeout_ms: 100,
        breaker_half_open_max_calls: 1,

    }
}

fn client() -> Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}

/// 流式端点：返回 6 个 chunk 分块响应（模拟 SSE `data: ...\n\n` 形态）。
/// 上游总 body 约 60 字节；call_streaming 调用方应能立即读到 chunk，
/// 不应等到全部 60 字节到齐。
#[tokio::test]
async fn call_streaming_returns_chunks_without_buffer() {
    let app = axum::Router::new().route(
        "/stream",
        axum::routing::get(|| async {
            let stream = axum::body::Body::from_stream(futures::stream::iter(vec![
                Ok::<_, axum::Error>(Bytes::from_static(b"chunk1\n")),
                Ok(Bytes::from_static(b"chunk2\n")),
                Ok(Bytes::from_static(b"chunk3\n")),
            ]));
            (
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                stream,
            )
        }),
    );
    let base = spawn_server(app).await;

    let rc = ResilientClient::new(client(), cfg(), Arc::new(NoopObserver));
    let mut headers = HeaderMap::new();
    headers.insert("accept", "text/event-stream".parse().unwrap());

    let res = rc
        .call_streaming(
            "test",
            Method::GET,
            &format!("{base}/stream"),
            &headers,
            Bytes::new(),
        )
        .await
        .expect("stream call ok");
    assert!(res.status.is_success());
    assert_eq!(
        res.headers
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );

    use futures::StreamExt;
    let body = res.body;
    let mut stream = body.bytes_stream();
    let mut collected = Vec::new();
    while let Some(chunk) = stream.next().await {
        collected.push(chunk.unwrap());
    }
    let joined: Vec<u8> = collected.iter().flat_map(|b| b.iter().copied()).collect();
    assert_eq!(
        String::from_utf8(joined).unwrap(),
        "chunk1\nchunk2\nchunk3\n"
    );
}

/// 上游 5xx 不应 retry（流式已 no-retry 策略），仅第一次响应计入 breaker。
#[tokio::test]
async fn call_streaming_records_5xx_failure_without_retry() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let count = Arc::new(AtomicUsize::new(0));
    let c2 = count.clone();
    let app = axum::Router::new().route(
        "/bad",
        axum::routing::get(move || {
            let c = c2.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            }
        }),
    );
    let base = spawn_server(app).await;

    let rc = ResilientClient::new(client(), cfg(), Arc::new(NoopObserver));
    let mut headers = HeaderMap::new();
    headers.insert("accept", "*/*".parse().unwrap());

    let res = rc
        .call_streaming(
            "test",
            Method::GET,
            &format!("{base}/bad"),
            &headers,
            Bytes::new(),
        )
        .await
        .expect("stream call returns 5xx as Ok");
    assert_eq!(res.status.as_u16(), 500);
    // 关键断言：不 retry（已订立的流不可 replay）
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
