//! Per-request tracing middleware.
//!
//! Creates a root span for each HTTP request, extracts W3C `traceparent`
//! header if present (so we chain into upstream traces), and emits a
//! `trace_id` field in the response `X-Trace-Id` header for client correlation.

use axum::{
    body::Body,
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use opentelemetry::global;
use opentelemetry::trace::{TraceContextExt, TraceId};
use std::time::Instant;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Per-request root span. Adds:
/// - `http.method`, `http.path`, `http.route` fields
/// - `trace_id` (32-hex W3C trace id) on the response as `X-Trace-Id`
/// - downstream calls: pass `traceparent` / `tracestate` if request had one
pub async fn trace_request_middleware(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let span = tracing::info_span!(
        "http_request",
        http.method = %method,
        http.path = %path,
        http.status_code = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
        otel.kind = "server",
    );

    // If the incoming request carried a W3C traceparent header, link our span
    // to the upstream trace context so distributed traces chain correctly.
    {
        use opentelemetry::propagation::Extractor;
        struct Hd<'a>(&'a axum::http::HeaderMap);
        impl<'a> Extractor for Hd<'a> {
            fn get(&self, key: &str) -> Option<&str> {
                self.0.get(key).and_then(|v| v.to_str().ok())
            }
            fn keys(&self) -> Vec<&str> {
                self.0.keys().map(|k| k.as_str()).collect()
            }
        }
        let cx = global::get_text_map_propagator(|prop| prop.extract(&Hd(request.headers())));
        span.set_parent(cx);
    }

    let start = Instant::now();
    let mut response = next.run(request).instrument(span.clone()).await;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    // Pull trace_id from the span context and inject into response.
    // If OTel subscriber is set up and the span has a real trace_id, use it.
    // Otherwise generate a deterministic random one so the header is always present.
    let trace_id = {
        let cx = span.context();
        let span_ref = cx.span();
        let sc = span_ref.span_context();
        if sc.is_valid() {
            sc.trace_id().to_string()
        } else {
            // No OTel subscriber — fall back to a random 128-bit ID
            // so callers can still correlate via X-Trace-Id.
            let id = TraceId::from_bytes(rand::random::<[u8; 16]>());
            id.to_string()
        }
    };

    span.record("http.status_code", response.status().as_u16() as i64);
    span.record("duration_ms", elapsed_ms as i64);

    if !trace_id.is_empty() {
        if let Ok(hv) = HeaderValue::from_str(&trace_id) {
            response.headers_mut().insert("X-Trace-Id", hv);
        }
    }

    response
}