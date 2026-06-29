//! common-telemetry: tracing initialization with optional OTel export.
//!
//! Two modes:
//! - Default (no `otlp` feature): plain tracing-subscriber (JSON logs to stdout).
//! - With `otlp` feature + `OTEL_EXPORTER_OTLP_ENDPOINT` env: layer OTel gRPC
//!   exporter on top of tracing-subscriber. Spans appear in Jaeger / Tempo / OTel collector.

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
/// common_telemetry::init_tracing("audit-service");
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

#[cfg(feature = "axum")]
pub mod middleware;
#[cfg(feature = "axum")]
pub use middleware::trace_request_middleware;