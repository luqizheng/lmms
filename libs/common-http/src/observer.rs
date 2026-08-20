use crate::circuit_breaker::CircuitState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Timeout,
    ConnectError,
    UpstreamError,
    CircuitOpen,
}

/// 库定义契约，调用方实现写监控（决策 3A）。默认方法为空，
/// 仅关心部分事件的实现可只覆盖需要的方法。
pub trait ResilienceObserver: Send + Sync {
    fn on_state_change(&self, _service: &str, _state: CircuitState) {}
    fn on_outcome(&self, _service: &str, _outcome: Outcome) {}
    fn on_retry(&self, _service: &str) {}
}

/// 零成本默认实现，供不接监控的调用方使用。
pub struct NoopObserver;
impl ResilienceObserver for NoopObserver {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingObserver {
        states: Mutex<Vec<(String, CircuitState)>>,
        outcomes: Mutex<Vec<(String, Outcome)>>,
        retries: Mutex<Vec<String>>,
    }
    impl ResilienceObserver for RecordingObserver {
        fn on_state_change(&self, service: &str, state: CircuitState) {
            self.states.lock().unwrap().push((service.into(), state));
        }
        fn on_outcome(&self, service: &str, outcome: Outcome) {
            self.outcomes
                .lock()
                .unwrap()
                .push((service.into(), outcome));
        }
        fn on_retry(&self, service: &str) {
            self.retries.lock().unwrap().push(service.into());
        }
    }

    #[test]
    fn recording_observer_captures_events() {
        let o = RecordingObserver::default();
        o.on_outcome("iam", Outcome::Success);
        o.on_state_change("iam", CircuitState::Open);
        o.on_retry("iam");
        assert_eq!(o.outcomes.lock().unwrap().len(), 1);
        assert_eq!(o.states.lock().unwrap()[0].1, CircuitState::Open);
        assert_eq!(o.retries.lock().unwrap()[0], "iam");
    }

    #[test]
    fn noop_observer_is_silent() {
        let o = NoopObserver;
        o.on_outcome("x", Outcome::Timeout);
    }
}
