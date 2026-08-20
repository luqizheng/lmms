//! Test that the axum middleware emits a JSON access-log event with
//! `trace_id` as a top-level field readable by Promtail's `json` stage.
//!
//! Default-features build (no `otlp`) exercises `fallback_impl`, which
//! generates a random 32-char hex `trace_id` per request. We capture the
//! JSON output via a custom `MakeWriter`, send a `/health` request, and
//! assert the captured bytes contain `"trace_id":"<32 hex chars>"`.

use axum::{routing::get, Router};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Shared buffer + clone-able writer so the subscriber can keep a clone
/// while the test still owns the `Arc` and can read what was written.
#[derive(Clone)]
struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl CaptureWriter {
    fn new() -> (Self, Arc<Mutex<Vec<u8>>>) {
        let buf = Arc::new(Mutex::new(Vec::new()));
        (Self(buf.clone()), buf)
    }
}

impl std::io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CaptureWriter {
    type Writer = CaptureWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn access_log_contains_trace_id() {
    let (writer, buf) = CaptureWriter::new();

    // `try_init` is fine: this is the only test in the binary, so the
    // global subscriber slot is empty. If another integration test in
    // the same package later shares the process, `.ok()` swallows the
    // AlreadyInitialized error and we still get useful output.
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::new("info"))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_writer(writer),
        )
        .try_init();

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn(
            digua_common_telemetry_axum::trace_request_middleware,
        ));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);

    // The X-Trace-Id header set by the middleware should also be a 32-hex string
    // (the same id used in the log event).
    let trace_header = response
        .headers()
        .get("x-trace-id")
        .expect("middleware should set X-Trace-Id")
        .to_str()
        .unwrap();
    assert_eq!(
        trace_header.len(),
        32,
        "X-Trace-Id should be 32 hex chars, got: {trace_header}"
    );
    assert!(
        trace_header.chars().all(|c| c.is_ascii_hexdigit()),
        "X-Trace-Id should be hex, got: {trace_header}"
    );

    let output = String::from_utf8(buf.lock().unwrap().clone())
        .expect("captured output should be valid UTF-8");
    println!("Captured JSON output:\n{output}");

    // The `tracing::info!(target: "http_access", ...)` event should appear
    // as one JSON object per request with `trace_id` as a top-level field.
    // Without `with_span_events(CLOSE)` enabled, span events are NOT emitted
    // to stdout, so the only JSON line carrying `trace_id` is our access
    // log event.
    let trace_id_re = regex::Regex::new(r#""trace_id":"([0-9a-f]{32})""#).unwrap();
    let caps = trace_id_re
        .captures(&output)
        .unwrap_or_else(|| panic!("expected `trace_id` JSON field in output, got:\n{output}"));
    let captured_trace_id = &caps[1];
    assert_eq!(
        captured_trace_id, trace_header,
        "log trace_id must match response X-Trace-Id header"
    );

    // Verify the access-log event uses the http_access target.
    assert!(
        output.contains(r#""target":"http_access""#),
        "expected access-log event with target=http_access, got:\n{output}"
    );
}
