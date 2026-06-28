#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Timeout,
    ConnectError,
    UpstreamError,
    CircuitOpen,
}

pub trait ResilienceObserver: Send + Sync {}

pub struct NoopObserver;
impl ResilienceObserver for NoopObserver {}