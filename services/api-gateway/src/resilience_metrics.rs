use common_http::{CircuitState, Outcome, ResilienceObserver};
use prometheus::{register_int_counter_vec, register_int_gauge_vec, IntCounterVec, IntGaugeVec};
use std::sync::OnceLock;

struct Metrics {
    circuit_state: IntGaugeVec,
    requests: IntCounterVec,
    retries: IntCounterVec,
}

fn metrics() -> &'static Metrics {
    static M: OnceLock<Metrics> = OnceLock::new();
    M.get_or_init(|| Metrics {
        circuit_state: register_int_gauge_vec!(
            "gateway_circuit_state",
            "Circuit breaker state per downstream service (0=Closed,1=HalfOpen,2=Open)",
            &["service"]
        )
        .unwrap(),
        requests: register_int_counter_vec!(
            "gateway_upstream_requests_total",
            "Upstream request outcomes per downstream service",
            &["service", "outcome"]
        )
        .unwrap(),
        retries: register_int_counter_vec!(
            "gateway_upstream_retries_total",
            "Upstream retries per downstream service",
            &["service"]
        )
        .unwrap(),
    })
}

fn state_code(s: CircuitState) -> i64 {
    match s {
        CircuitState::Closed => 0,
        CircuitState::HalfOpen => 1,
        CircuitState::Open => 2,
    }
}

fn outcome_label(o: Outcome) -> &'static str {
    match o {
        Outcome::Success => "success",
        Outcome::Timeout => "timeout",
        Outcome::ConnectError => "connect_error",
        Outcome::UpstreamError => "upstream_error",
        Outcome::CircuitOpen => "circuit_open",
    }
}

/// gateway 侧 observer：把韧性事件写入默认 prometheus registry，
/// 经现有 /metrics 端点暴露（决策 3A）。
pub struct GatewayMetricsObserver;

impl ResilienceObserver for GatewayMetricsObserver {
    fn on_state_change(&self, service: &str, state: CircuitState) {
        metrics()
            .circuit_state
            .with_label_values(&[service])
            .set(state_code(state));
    }
    fn on_outcome(&self, service: &str, outcome: Outcome) {
        metrics()
            .requests
            .with_label_values(&[service, outcome_label(outcome)])
            .inc();
    }
    fn on_retry(&self, service: &str) {
        metrics().retries.with_label_values(&[service]).inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_records_without_panic() {
        let o = GatewayMetricsObserver;
        o.on_outcome("iam", Outcome::Success);
        o.on_state_change("iam", CircuitState::Open);
        o.on_retry("iam");
        let txt = {
            use prometheus::{Encoder, TextEncoder};
            let mut buf = Vec::new();
            TextEncoder::new()
                .encode(&prometheus::gather(), &mut buf)
                .unwrap();
            String::from_utf8(buf).unwrap()
        };
        assert!(txt.contains("gateway_circuit_state"));
        assert!(txt.contains("gateway_upstream_requests_total"));
    }
}
