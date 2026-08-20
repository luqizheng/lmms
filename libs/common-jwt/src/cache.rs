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
        self.inner
            .try_read()
            .ok()
            .and_then(|g| g.keys.get(kid).cloned())
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

    pub fn spawn_refresh_loop(self: Arc<Self>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
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

    /// Startup helper: 阻塞重试 refresh 直到成功或超过 `startup_max_wait`，
    /// 然后 spawn 后台定时刷新 loop（接收 shutdown 信号优雅退出）。
    ///
    /// 设计动机：避免 docker compose 启动顺序竞态 —— token-service 还没 listening
    /// 时其他服务调一次 `refresh()` 失败被 warn 忽略，之后 `verify_token` 永远拿不到
    /// default key，所有受保护端点返回 401。
    ///
    /// 重试策略：指数退避 100ms → 200ms → 400ms → ... → 5s 封顶；超时后**不再阻塞
    /// 服务启动**，继续以 stale 空 cache 起来并让后台 loop 继续重试（与原 warn-and-
    /// continue 行为兼容，但启动成功时立即可用）。
    ///
    /// 返回 shutdown sender：调用方在收到 SIGTERM/SIGINT 时 `send(true)`，后台 loop
    /// 即退出。
    ///
    /// # Example
    /// ```ignore
    /// let jwks = Arc::new(JWKSCache::new(cfg, http));
    /// let jwks_shutdown = jwks
    ///     .init_with_refresh_loop(Duration::from_secs(30))
    ///     .await;
    /// // ... 在 ctrl_c handler 里: let _ = jwks_shutdown.send(true);
    /// ```
    pub async fn init_with_refresh_loop(
        self: Arc<Self>,
        startup_max_wait: Duration,
    ) -> tokio::sync::watch::Sender<bool> {
        let deadline = Instant::now() + startup_max_wait;
        let mut backoff = Duration::from_millis(100);
        let max_backoff = Duration::from_secs(5);
        let mut attempt: u32 = 0;

        loop {
            attempt += 1;
            match self.refresh().await {
                Ok(()) => {
                    tracing::info!(
                        attempts = attempt,
                        url = %self.config.url,
                        "JWKS initial refresh succeeded"
                    );
                    break;
                }
                Err(e) => {
                    if Instant::now() >= deadline {
                        tracing::error!(
                            attempts = attempt,
                            waited = ?startup_max_wait,
                            url = %self.config.url,
                            error = %e,
                            "JWKS initial refresh still failing after max wait; \
                             continuing with stale/empty cache, background loop will keep retrying"
                        );
                        break;
                    }
                    tracing::warn!(
                        attempts = attempt,
                        next_retry_in = ?backoff,
                        url = %self.config.url,
                        error = %e,
                        "JWKS initial refresh failed, will retry"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(max_backoff);
                }
            }
        }

        let (tx, rx) = tokio::sync::watch::channel(false);
        self.spawn_refresh_loop(rx);
        tx
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
    let parsed: Jwks =
        serde_json::from_str(json).map_err(|e| JwksError::ParseFailed(e.to_string()))?;
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
