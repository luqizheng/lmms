use bytes::Bytes;
use common_http::ResilienceConfig;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn streaming_forward_passes_chunks_through() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use futures::StreamExt;
    use tokio::net::TcpListener;

    let app = axum::Router::new().route(
        "/stream",
        get(|| async {
            (
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                Body::from_stream(futures::stream::iter(vec![
                    Ok::<_, axum::Error>(Bytes::from_static(b"data: 1\n\n")),
                    Ok(Bytes::from_static(b"data: 2\n\n")),
                    Ok(Bytes::from_static(b"data: 3\n\n")),
                ])),
            )
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{addr}");

    let cfg = api_gateway::router::RouterConfig {
        resilient: Arc::new(common_http::ResilientClient::new(
            reqwest::Client::new(),
            ResilienceConfig::default(),
            Arc::new(common_http::NoopObserver),
        )),
        egress_map: Arc::new(HashMap::from([(
            "test".into(),
            api_gateway::config::EgressTarget {
                host: addr.ip().to_string(),
                port: addr.port(),
            },
        )])),
    };

    let req = Request::builder()
        .uri("/test/stream")
        .body(Body::empty())
        .unwrap();
    let resp = api_gateway::router::forward_to_service(
        Arc::new(cfg),
        req,
        "test",
        "/stream",
        true, // ws=true → streaming path
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let mut body = resp.into_body().into_data_stream();
    let mut collected = Vec::new();
    while let Some(chunk) = body.next().await {
        collected.push(chunk.unwrap().to_vec());
    }
    let joined: Vec<u8> = collected.iter().flat_map(|b| b.iter().copied()).collect();
    let text = String::from_utf8(joined).unwrap();
    assert!(text.contains("data: 1"));
    assert!(text.contains("data: 2"));
    assert!(text.contains("data: 3"));
}

#[tokio::test]
async fn non_ws_forward_buffers_response() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tokio::net::TcpListener;

    let app = axum::Router::new().route(
        "/x",
        get(|| async { (StatusCode::OK, "hello") }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let cfg = api_gateway::router::RouterConfig {
        resilient: Arc::new(common_http::ResilientClient::new(
            reqwest::Client::new(),
            ResilienceConfig::default(),
            Arc::new(common_http::NoopObserver),
        )),
        egress_map: Arc::new(HashMap::from([(
            "test".into(),
            api_gateway::config::EgressTarget {
                host: addr.ip().to_string(),
                port: addr.port(),
            },
        )])),
    };

    let req = Request::builder()
        .uri("/test/x")
        .body(Body::empty())
        .unwrap();
    let resp = api_gateway::router::forward_to_service(
        Arc::new(cfg),
        req,
        "test",
        "/x",
        false, // ws=false → buffered
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body_bytes[..], b"hello");
}