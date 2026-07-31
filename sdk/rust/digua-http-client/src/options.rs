use std::time::Duration;

#[derive(Debug, Clone)]
pub struct KudiHttpOptions {
    pub token_endpoint: String,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub retry_count: u32,
    pub refresh_skew: Duration,
    pub user_agent: String,
}

impl Default for KudiHttpOptions {
    fn default() -> Self {
        Self {
            token_endpoint: "http://token-svc/api/tokens/access".to_string(),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(15),
            retry_count: 1,
            refresh_skew: Duration::from_secs(60),
            user_agent: format!("kudi-http-client/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}
