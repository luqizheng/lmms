//! digua_common-telemetry-axum: axum request-tracing middleware.
//!
//! Split out from `digua_common-telemetry` so that CLI binaries (outbox-publisher
//! etc.) that only want `init_tracing` don't drag in the full axum / hyper /
//! tower / tower-http tree. See plan §3 decision row 5 ("outbox-publisher
//! binary 不加 axum feature").
//!
//! Two implementations gated by the `otlp` feature:
//! - `otlp` ON  → OTel SDK + tracing-opentelemetry. trace_id from real span
//!   context, falls back to a random 128-bit id if no OTel subscriber.
//! - `otlp` OFF → plain tracing root span + random 128-bit hex id.

#[cfg(feature = "otlp")]
mod otlp_impl {
    //! OTel implementation. Matches the original
    //! `digua_common-telemetry::middleware::otlp_impl` 1:1.
    use axum::{
        body::Body, extract::Request, http::HeaderValue, middleware::Next, response::Response,
    };
    use opentelemetry::global;
    use opentelemetry::trace::{TraceContextExt, TraceId};
    use std::time::Instant;
    use tracing::Instrument;

    /// 小写 hex 编码（输出与旧 hex::encode 一致）。内联实现以避免为单个
    /// fallback 调用点引入整条 hex 依赖链；otlp 开启时本模块被 cfg 掉，
    /// 故允许 dead_code。
    #[allow(dead_code)]
    fn hex_encode(bytes: &[u8; 16]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(32);
        for &b in bytes {
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
        out
    }
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    pub async fn trace_request_middleware(request: Request<Body>, next: Next) -> Response {
        let method = request.method().clone();
        let path = request.uri().path().to_string();

        let span = tracing::info_span!(
            "http_request",
            trace_id = tracing::field::Empty,
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
        span.record("trace_id", trace_id.as_str());

        // Emit one access-log event per HTTP request with trace_id as a top-level
        // field. Promtail parses JSON and Loki indexes `trace_id` directly.
        tracing::info!(
            target: "http_access",
            trace_id = %trace_id,
            http.method = %method,
            http.path = %path,
            http.status_code = response.status().as_u16(),
            duration_ms = elapsed_ms,
            "request completed"
        );

        if !trace_id.is_empty() {
            if let Ok(hv) = HeaderValue::from_str(&trace_id) {
                response.headers_mut().insert("X-Trace-Id", hv);
            }
        }

        response
    }
}

#[cfg(not(feature = "otlp"))]
mod fallback_impl {
    //! No-OTel fallback: root span + random 128-bit hex trace id.
    use axum::{
        body::Body, extract::Request, http::HeaderValue, middleware::Next, response::Response,
    };
    use std::time::Instant;
    use tracing::Instrument;

    /// 小写 hex 编码（输出与旧 hex::encode 一致）。内联实现以避免为单个
    /// fallback 调用点引入整条 hex 依赖链；otlp 开启时本模块被 cfg 掉，
    /// 故允许 dead_code。
    #[allow(dead_code)]
    fn hex_encode(bytes: &[u8; 16]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(32);
        for &b in bytes {
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
        out
    }

    pub async fn trace_request_middleware(request: Request<Body>, next: Next) -> Response {
        let method = request.method().clone();
        let path = request.uri().path().to_string();
        let span = tracing::info_span!(
            "http_request",
            trace_id = tracing::field::Empty,
            http.method = %method,
            http.path = %path,
            http.status_code = tracing::field::Empty,
            duration_ms = tracing::field::Empty,
        );
        let start = Instant::now();
        let mut response = next.run(request).instrument(span.clone()).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let trace_id = hex_encode(&rand::random::<[u8; 16]>());
        span.record("http.status_code", response.status().as_u16() as i64);
        span.record("duration_ms", elapsed_ms as i64);
        span.record("trace_id", trace_id.as_str());

        // Emit one access-log event per HTTP request with trace_id as a top-level
        // field. Promtail parses JSON and Loki indexes `trace_id` directly.
        tracing::info!(
            target: "http_access",
            trace_id = %trace_id,
            http.method = %method,
            http.path = %path,
            http.status_code = response.status().as_u16(),
            duration_ms = elapsed_ms,
            "request completed"
        );

        if let Ok(hv) = HeaderValue::from_str(&trace_id) {
            response.headers_mut().insert("X-Trace-Id", hv);
        }
        response
    }
}

#[cfg(feature = "otlp")]
pub use otlp_impl::trace_request_middleware;

#[cfg(not(feature = "otlp"))]
pub use fallback_impl::trace_request_middleware;
