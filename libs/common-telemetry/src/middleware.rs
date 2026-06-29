//! Per-request tracing middleware. Extracted from 4 copy-paste copies in
//! services/{api-gateway,iam-service,notification-service,token-service}.
//!
//! Module is gated by the `axum` feature — only HTTP-server consumers need it.
//! CLI binaries (outbox-publisher, etc.) that just want init_tracing leave it off.
//!
//! Two implementations:
//! - `otlp` feature ON  → OTel SDK + tracing-opentelemetry. Trace_id from real
//!   span context; falls back to a random 128-bit id if no OTel subscriber.
//! - `otlp` feature OFF → plain tracing root span + random 128-bit fallback id.

#[cfg(all(feature = "axum", feature = "otlp"))]
pub use otlp_impl::trace_request_middleware;

#[cfg(all(feature = "axum", not(feature = "otlp")))]
pub use fallback_impl::trace_request_middleware;

#[cfg(all(feature = "axum", feature = "otlp"))]
mod otlp_impl {
    //! 与 services/api-gateway/src/middleware/tracing_mw.rs 完全一致的 OTel 实现。
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

        let trace_id = {
            let cx = span.context();
            let span_ref = cx.span();
            let sc = span_ref.span_context();
            if sc.is_valid() {
                sc.trace_id().to_string()
            } else {
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
}

#[cfg(all(feature = "axum", not(feature = "otlp")))]
mod fallback_impl {
    //! 无 OTel：仅日志 root span + 随机 X-Trace-Id（不连 OTel collector）。
    use axum::{
        body::Body,
        extract::Request,
        http::HeaderValue,
        middleware::Next,
        response::Response,
    };
    use std::time::Instant;
    use tracing::Instrument;

    pub async fn trace_request_middleware(request: Request<Body>, next: Next) -> Response {
        let method = request.method().clone();
        let path = request.uri().path().to_string();
        let span = tracing::info_span!(
            "http_request",
            http.method = %method,
            http.path = %path,
            http.status_code = tracing::field::Empty,
            duration_ms = tracing::field::Empty,
        );
        let start = Instant::now();
        let mut response = next.run(request).instrument(span.clone()).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let trace_id = hex::encode(rand::random::<[u8; 16]>());
        span.record("http.status_code", response.status().as_u16() as i64);
        span.record("duration_ms", elapsed_ms as i64);
        if let Ok(hv) = HeaderValue::from_str(&trace_id) {
            response.headers_mut().insert("X-Trace-Id", hv);
        }
        response
    }
}
