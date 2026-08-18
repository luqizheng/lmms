//! digua_common-telemetry: tracing initialization with optional OTel export.
//!
//! Two modes:
//! - Default (no `otlp` feature): plain tracing-subscriber (JSON logs to stdout).
//! - With `otlp` feature + `OTEL_EXPORTER_OTLP_ENDPOINT` env: layer OTel gRPC
//!   exporter on top of tracing-subscriber. Spans appear in Jaeger / Tempo / OTel collector.
//!
//! The axum HTTP request-tracing middleware lives in the sibling crate
//! `digua_common-telemetry-axum` so that CLI binaries (outbox-publisher etc.) that
//! only want `init_tracing` don't pull in the full axum / hyper / tower
//! dependency tree.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing. Safe to call once per process.
///
/// Behavior:
/// 1. Always sets up a JSON-ish stdout layer.
/// 2. If `otlp` feature is enabled AND `OTEL_EXPORTER_OTLP_ENDPOINT` is set,
///    adds a tracing-opentelemetry layer exporting via OTLP/gRPC.
///
/// # Example
///
/// ```no_run
/// digua_common_telemetry::init_tracing("audit-service");
/// ```
pub fn init_tracing(service_name: &'static str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,sqlx=warn,hyper=warn,reqwest=warn,tower=warn")
    });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_level(true)
        .json();

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    #[cfg(feature = "otlp")]
    {
        use opentelemetry::{global, KeyValue};
        use opentelemetry::trace::TracerProvider as _;
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::{propagation::TraceContextPropagator, trace as sdktrace, Resource};
        use tracing_opentelemetry::OpenTelemetryLayer;

        if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            global::set_text_map_propagator(TraceContextPropagator::new());

            let resource = Resource::new(vec![KeyValue::new("service.name", service_name)]);

            let span_exporter = opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint.clone());

            let provider = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(span_exporter)
                .with_trace_config(sdktrace::Config::default().with_resource(resource))
                .install_batch(opentelemetry_sdk::runtime::Tokio)
                .expect("OTel pipeline install failed");

            let tracer = provider.tracer(service_name);
            let otel_layer = OpenTelemetryLayer::new(tracer);
            subscriber.with(otel_layer).try_init().ok();
            tracing::info!(
                target: "telemetry",
                service = service_name,
                "OTel tracing enabled → {}",
                endpoint
            );
            return;
        }
    }

    #[cfg(not(feature = "otlp"))]
    {
        let _ = service_name; // suppress unused warning when otlp is off
    }

    subscriber.try_init().ok();
    tracing::info!(
        target: "telemetry",
        service = service_name,
        "tracing initialized (stdout only, no OTLP)"
    );
}

/// Emit a shutdown hook that flushes any pending OTel spans.
///
/// Call before process exit if OTel is enabled.
pub fn shutdown() {
    #[cfg(feature = "otlp")]
    {
        opentelemetry::global::shutdown_tracer_provider();
    }
}

mod trace_id;
pub use trace_id::current_trace_id;

#[cfg(all(test, feature = "otlp"))]
mod otlp_exporter_registration_tests {
    use super::init_tracing;

    /// P0 守门测试：`otlp` feature + `OTEL_EXPORTER_OTLP_ENDPOINT` 时，
    /// `init_tracing` 必须真正走 OTLP 分支——注册 exporter pipeline 并安装
    /// W3C TraceContext 传播器。
    ///
    /// 背景（2026-08 iam-service P0）：workspace 用 resolver="2"，iam-service
    /// 的 Dockerfile 单独 `cargo build -p iam_service`，依赖闭包内只有它自己
    /// 声明 digua_common_telemetry（曾漏掉 `features = ["otlp"]`），导致
    /// `#[cfg(feature = "otlp")]` 分支被整体编译掉，span 永远到不了 Jaeger。
    /// 若未来再有服务摘掉 otlp feature 声明，此测试会被 cfg 静默排除——
    /// 因此配合 scripts/check-service-standard.js 的静态守门双保险。
    ///
    /// 断言依据：`global::set_text_map_propagator(TraceContextPropagator)`
    /// 只在 otlp 分支执行（见 init_tracing），且 install_batch() 失败会 panic。
    /// 走到 propagator 断言 ⇒ exporter pipeline 构建成功。
    #[tokio::test(flavor = "multi_thread")]
    async fn init_tracing_registers_otlp_pipeline() {
        // 指向不可达地址：install_batch() 不阻塞（exporter 后台重试），
        // 只要能完成安装即证明 OTLP pipeline 已接线。
        std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:1");

        init_tracing("telemetry-test-svc");

        let otlp_pipeline_registered = opentelemetry::global::get_text_map_propagator(|p| {
            // W3C TraceContextPropagator.fields() == ["traceparent", "tracestate"]；
            // 未注册时为 no-op propagator，fields() 为空。
            p.fields().map(|f| f.to_string()).collect::<Vec<_>>()
        });
        assert_eq!(
            otlp_pipeline_registered,
            ["traceparent", "tracestate"],
            "init_tracing with otlp feature + OTEL_EXPORTER_OTLP_ENDPOINT must register the \
             OTLP pipeline (TraceContextPropagator + exporter); otherwise spans never reach \
             Jaeger (iam-service P0 regression)"
        );

        super::shutdown();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}
