//! 纯函数测：current_trace_id() 在三种 span context 下返回正确结果。
//! 用 `tracing::subscriber::with_default(...)` scope 隔离，避免全局 provider 污染。

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::TracerProvider;
use tracing_subscriber::layer::SubscriberExt;

fn run_with_otel_layer<F: FnOnce() -> R, R>(f: F) -> R {
    let provider = TracerProvider::builder().build();
    let tracer = provider.tracer("test");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = tracing_subscriber::registry().with(otel_layer);
    tracing::subscriber::with_default(subscriber, f)
}

#[cfg(feature = "otlp")]
#[test]
fn returns_trace_id_inside_active_span() {
    run_with_otel_layer(|| {
        let _g = tracing::info_span!("test").entered();
        let tid = digua_common_telemetry::current_trace_id();
        assert!(tid.is_some(), "expected Some(trace_id) inside active span");
    });
}

#[test]
fn returns_none_with_otel_subscriber_but_no_span() {
    run_with_otel_layer(|| {
        assert!(digua_common_telemetry::current_trace_id().is_none());
    });
}

#[test]
fn returns_none_without_subscriber() {
    assert!(digua_common_telemetry::current_trace_id().is_none());
}