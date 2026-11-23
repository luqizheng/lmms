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

/// Legacy kid hard-coded by the pre-keystore era (`sign_token` still stamps
/// it for backward compatibility). Keystore-driven signers use fingerprint
/// kids and should pass them explicitly via `sign_token_with_kid`.
pub const LEGACY_KID: &str = "default";

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

    /// Atomically replace the cached key set (kid → DecodingKey). Used by
    /// in-process key rotation (token-service keystore): the service is its
    /// own JWKS authority, so after rotate/retire it re-installs the key set
    /// locally instead of waiting for the next HTTP refresh. Holders of the
    /// existing `Arc<JWKSCache>` see the new keys immediately.
    pub async fn install_keys(&self, keys: HashMap<String, DecodingKey>) {
        let mut g = self.inner.write().await;
        g.keys = keys;
        g.fetched_at = Some(Instant::now());
    }

    /// `install_keys` variant taking (kid, public PEM) pairs — the shape the
    /// keystore publishes. Fails without mutating anything if any PEM is
    /// unparseable (fail-closed, matches keystore load semantics).
    pub async fn install_from_pems(&self, entries: &[(String, String)]) -> Result<(), JwksError> {
        let mut keys = HashMap::with_capacity(entries.len());
        for (kid, pem) in entries {
            let key = DecodingKey::from_rsa_pem(pem.as_bytes())
                .map_err(|e| JwksError::KeyBuildFailed(format!("kid {kid}: {e}")))?;
            keys.insert(kid.clone(), key);
        }
        self.install_keys(keys).await;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::{sign_token, sign_token_with_kid, verify_token};

    fn pair() -> (jwt_simple::prelude::RS256KeyPair, String, String) {
        let kp = jwt_simple::prelude::RS256KeyPair::generate(2048).unwrap();
        let priv_pem = kp.to_pem().unwrap();
        let pub_pem = kp.public_key().to_pem().unwrap();
        (kp, priv_pem, pub_pem)
    }

    #[tokio::test]
    async fn sign_token_with_kid_stamps_header_and_verifies_via_installed_key() {
        let (_kp, priv_pem, pub_pem) = pair();
        let cache = JWKSCache::new(Default::default(), reqwest::Client::new());
        cache
            .install_from_pems(&[("rotated-1".to_string(), pub_pem)])
            .await
            .unwrap();

        let token = sign_token_with_kid(
            &priv_pem,
            "rotated-1",
            &serde_json::json!({"sub": "u1"}),
            std::time::Duration::from_secs(60),
        )
        .unwrap();
        let claims = verify_token(&token, &cache, None).await.unwrap();
        assert_eq!(claims["sub"], "u1");

        // kid-strict: a token claiming a kid that is not installed fails
        let stale = sign_token_with_kid(
            &priv_pem,
            "never-installed",
            &serde_json::json!({"sub": "u1"}),
            std::time::Duration::from_secs(60),
        )
        .unwrap();
        assert!(verify_token(&stale, &cache, None).await.is_err());
    }

    #[tokio::test]
    async fn install_from_pems_is_atomic_on_bad_input() {
        let (_kp, priv_pem, pub_pem) = pair();
        let cache = JWKSCache::new(Default::default(), reqwest::Client::new());
        cache
            .install_from_pems(&[("good".to_string(), pub_pem.clone())])
            .await
            .unwrap();

        let bad = vec![
            ("good".to_string(), pub_pem),
            ("bad".to_string(), "not-a-pem".to_string()),
        ];
        assert!(cache.install_from_pems(&bad).await.is_err());
        // old key set untouched: the "good" kid still verifies
        let token = sign_token_with_kid(
            &priv_pem,
            "good",
            &serde_json::json!({"sub": "u1"}),
            std::time::Duration::from_secs(60),
        )
        .unwrap();
        assert!(verify_token(&token, &cache, None).await.is_ok());
    }

    #[tokio::test]
    async fn legacy_sign_token_verifies_via_default_kid_install() {
        let (_kp, priv_pem, pub_pem) = pair();
        let cache = JWKSCache::from_pem(&pub_pem).unwrap();
        let token = sign_token(
            &priv_pem,
            &serde_json::json!({"sub": "u1"}),
            std::time::Duration::from_secs(60),
        )
        .unwrap();
        assert!(verify_token(&token, &cache, None).await.is_ok());
    }
}
