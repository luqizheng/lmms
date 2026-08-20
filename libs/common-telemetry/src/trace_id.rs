//! 从当前 tracing span 取 W3C TraceId 字符串。
//!
//! 与 trace_request_middleware（middleware.rs）的 trace_id 取法对齐：
//! 通过 tracing-opentelemetry 的 `OpenTelemetrySpanExt::context()`
//! 拿到 OTel context，再从 span_context() 取 trace_id。
//! `is_valid()` 为 false（未接 OTel / 无 root span / 测试场景）时返回 None，绝不伪造。

#[cfg(feature = "otlp")]
pub fn current_trace_id() -> Option<String> {
    use opentelemetry::trace::TraceContextExt;
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    let span = tracing::Span::current();
    let cx = span.context();
    let sc = cx.span().span_context().clone();
    if sc.is_valid() {
        Some(sc.trace_id().to_string())
    } else {
        None
    }
}

#[cfg(not(feature = "otlp"))]
pub fn current_trace_id() -> Option<String> {
    None
}
