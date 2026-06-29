//! Consumer-side OTel span parent 关联。
//!
//! Phase 6-A T2 参考实现：从 envelope.trace_id (T1 已注入) 建一个 trace context
//! 作为 consumer span 的 parent，让 producer→consumer 跨服务 chain 在 Jaeger 可见。
//!
//! 仅 audit 一处使用。notification / gateway perm_consumer 留 T2-fu1 follow-up。

use opentelemetry::{
    trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState},
    Context,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use common_event::Envelope;
use common_event::event::Event;

/// Build a tracing span whose OTel parent points to the producer's trace.
/// If envelope.trace_id is None or malformed, the span is parentless
/// (graceful degradation, not error).
pub fn span_from_envelope_trace_id<P: Event>(
    env: &Envelope<P>,
    span_name: &'static str,
) -> tracing::Span {
    let span = tracing::info_span!(
        target: "consumer",
        "consume",
        event_type = %env.event_type,
        otel.kind = "consumer",
        _span_name = span_name,
    );
    if let Some(tid_hex) = env.trace_id.as_deref() {
        if let Ok(tid) = TraceId::from_hex(tid_hex) {
            let sc = SpanContext::new(
                tid,
                SpanId::INVALID,
                TraceFlags::SAMPLED,
                /*is_remote=*/ true,
                TraceState::default(),
            );
            let cx = Context::current().with_remote_span_context(sc);
            span.set_parent(cx);
        }
    }
    span
}
