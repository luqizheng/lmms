use jsonwebtoken::DecodingKey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum JwksError {
    #[error("JWKS fetch failed: {0}")]
    FetchFailed(#[from] reqwest::Error),
    #[error("JWKS parse failed: {0}")]
    ParseFailed(String),
    #[error("missing kid in JWK")]
    MissingKid,
    #[error("JWK construction failed: {0}")]
    KeyBuildFailed(String),
}

#[derive(Clone, Debug)]
pub struct JwksConfig {
    pub url: String,
    pub refresh_interval: Duration,
    pub request_timeout: Duration,
}

impl Default for JwksConfig {
    fn default() -> Self {
        Self {
            url: "http://token-service:8082/.well-known/jwks.json".into(),
            refresh_interval: Duration::from_secs(3600),
            request_timeout: Duration::from_secs(5),
        }
    }
}

pub struct JwksInner {
    pub keys: HashMap<String, DecodingKey>,
    pub fetched_at: Option<Instant>,
}

pub struct JWKSCache {
    pub inner: Arc<RwLock<JwksInner>>,
    pub config: JwksConfig,
    pub http: reqwest::Client,
}

impl JWKSCache {
    pub fn new(config: JwksConfig, http: reqwest::Client) -> Self {
        Self {
            inner: Arc::new(RwLock::new(JwksInner {
                keys: HashMap::new(),
                fetched_at: None,
            })),
            config,
            http,
        }
    }

    /// Test-only constructor: pre-load a single public key with kid="default".
    /// Use this in unit tests; production code should use `new` + `refresh`.
    pub fn from_pem(public_pem: &str) -> Result<Self, JwksError> {
        let key = DecodingKey::from_rsa_pem(public_pem.as_bytes())
            .map_err(|e| JwksError::KeyBuildFailed(e.to_string()))?;
        let mut keys = HashMap::new();
        keys.insert("default".to_string(), key);
        Ok(Self {
            inner: Arc::new(RwLock::new(JwksInner {
                keys,
                fetched_at: Some(Instant::now()),
            })),
            config: JwksConfig {
                url: String::new(),
                refresh_interval: Duration::from_secs(3600),
                request_timeout: Duration::from_secs(5),
            },
            http: reqwest::Client::new(),
        })
    }

    pub fn get(&self, kid: &str) -> Option<DecodingKey> {
        self.inner.try_read().ok().and_then(|g| g.keys.get(kid).cloned())
    }

    /// Convenience: get the key with kid="default". None if not present.
    /// Use this when caller doesn't care about kid (single-key deployments).
    pub fn default_key(&self) -> Option<DecodingKey> {
        self.get("default")
    }

    pub async fn refresh(&self) -> Result<(), JwksError> {
        let resp = self
            .http
            .get(&self.config.url)
            .timeout(self.config.request_timeout)
            .send()
            .await?;
        let text = resp.error_for_status()?.text().await?;
        let keys = parse_jwks(&text)?;
        let mut g = self.inner.write().await;
        g.keys = keys;
        g.fetched_at = Some(Instant::now());
        Ok(())
    }

    pub fn spawn_refresh_loop(
        self: Arc<Self>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(self.config.refresh_interval) => {
                        match self.refresh().await {
                            Ok(()) => tracing::info!("jwks refreshed"),
                            Err(e) => tracing::warn!("jwks refresh failed: {}, keeping stale", e),
                        }
                    }
                    _ = shutdown.changed() => {
                        if *shutdown.borrow() { break; }
                    }
                }
            }
        });
    }
}

/// Parse a JWKS JSON document and build DecodingKeys keyed by `kid`.
/// Skips entries without a kid or with non-RSA kty.
pub fn parse_jwks(json: &str) -> Result<HashMap<String, DecodingKey>, JwksError> {
    #[derive(serde::Deserialize)]
    struct Jwks {
        keys: Vec<Jwk>,
    }
    #[derive(serde::Deserialize)]
    struct Jwk {
        kid: Option<String>,
        kty: String,
        n: Option<String>,
        e: Option<String>,
    }
    let parsed: Jwks = serde_json::from_str(json)
        .map_err(|e| JwksError::ParseFailed(e.to_string()))?;
    let mut out = HashMap::new();
    for jwk in parsed.keys {
        if jwk.kty != "RSA" {
            continue;
        }
        let kid = match jwk.kid {
            Some(k) => k,
            None => continue,
        };
        let n = jwk.n.ok_or(JwksError::MissingKid)?;
        let e = jwk.e.ok_or(JwksError::MissingKid)?;
        let key = DecodingKey::from_rsa_components(&n, &e)
            .map_err(|err| JwksError::KeyBuildFailed(err.to_string()))?;
        out.insert(kid, key);
    }
    Ok(out)
}
