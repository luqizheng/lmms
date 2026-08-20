//! Integration tests for the OTel tracing middleware.
//!
//! Verifies:
//! - Each response carries a 32-hex `X-Trace-Id` header
//! - Subsequent requests get different trace IDs (no fixed / all-zero leak)
//! - When a `traceparent` header is sent and an OTel subscriber is set,
//!   the span chains to the upstream trace (verified via Jaeger in CI)
//!
//! These tests run WITHOUT a real OTel subscriber; full propagation
//! correctness is validated end-to-end against Jaeger.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware as axum_mw;
use axum::routing::{get, Router};
use tower::util::ServiceExt;

async fn ok() -> &'static str {
    "OK"
}

fn app() -> Router {
    Router::new()
        .route("/test", get(ok))
        .layer(axum_mw::from_fn(
            digua_common_telemetry_axum::trace_request_middleware,
        ))
}

#[tokio::test]
async fn response_includes_trace_id_header() {
    let res = app()
        .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let trace_id = res.headers().get("X-Trace-Id");
    assert!(trace_id.is_some(), "X-Trace-Id header missing");
    let trace_id = trace_id.unwrap().to_str().unwrap();
    // W3C trace_id is 32 hex chars
    assert_eq!(trace_id.len(), 32, "trace_id should be 32 hex chars");
    assert!(
        trace_id.chars().all(|c| c.is_ascii_hexdigit()),
        "trace_id should be hex"
    );
}

#[tokio::test]
async fn each_request_gets_unique_trace_id() {
    let res1 = app()
        .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let res2 = app()
        .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let id1 = res1.headers().get("X-Trace-Id").unwrap().to_str().unwrap();
    let id2 = res2.headers().get("X-Trace-Id").unwrap().to_str().unwrap();
    assert_ne!(id1, id2, "each request should get a unique trace_id");
    assert_ne!(id1, "00000000000000000000000000000000");
}

#[tokio::test]
async fn traceparent_header_does_not_panic() {
    // The middleware should accept requests with W3C traceparent headers
    // without erroring. Full chain verification happens end-to-end with
    // an OTel subscriber active.
    let traceparent = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
    let res = app()
        .oneshot(
            Request::builder()
                .uri("/test")
                .header("traceparent", traceparent)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let trace_id = res.headers().get("X-Trace-Id");
    assert!(trace_id.is_some());
}
