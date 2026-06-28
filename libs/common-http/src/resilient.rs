pub struct UpstreamResponse;
pub enum ResilientError {
    CircuitOpen,
    Timeout,
    Connect(reqwest::Error),
}
pub struct ResilientClient;