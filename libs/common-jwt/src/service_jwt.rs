use crate::sign_token;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct ServiceJwtConfig {
    pub token_service_url: String,
    pub bootstrap_token: String,
    pub caller_service_name: String,
    pub caller_tenant: i64,
    pub refresh_skew: Duration,
    pub request_timeout: Duration,
}

#[derive(Error, Debug)]
pub enum ServiceJwtError {
    #[error("token-service request failed: {0}")]
    TokenServiceUnavailable(#[from] reqwest::Error),
    #[error("token-service response parse failed: {0}")]
    ParseFailed(String),
    #[error("auth failure: {0}")]
    AuthFailed(String),
    #[error("internal: {0}")]
    Internal(String),
}

struct ServiceJwtInner {
    tokens: HashMap<String, String>, // cache key -> JWT
    expires_at: HashMap<String, Instant>,
    /// Mint params per cache key, so the background refresh loop can re-mint
    /// without knowing how the key was created. Key = "target" (legacy
    /// fixed-tenant path) or "target:tid:ct" (per-tenant path).
    params: HashMap<String, (String, i64, bool)>,
}

pub struct ServiceJwtCache {
    inner: Arc<RwLock<ServiceJwtInner>>,
    config: ServiceJwtConfig,
    http: reqwest::Client,
    private_key_pem: String,
}

#[derive(Deserialize)]
struct MintResponse {
    access_token: String,
    expires_in: i64,
}

impl ServiceJwtCache {
    pub fn new(config: ServiceJwtConfig, http: reqwest::Client, private_key_pem: String) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ServiceJwtInner {
                tokens: HashMap::new(),
                expires_at: HashMap::new(),
                params: HashMap::new(),
            })),
            config,
            http,
            private_key_pem,
        }
    }

    /// Get service-jwt for a target. Triggers a refresh if cache empty or
    /// within `refresh_skew` of expiry.
    pub async fn get(&self, target: &str) -> Result<String, ServiceJwtError> {
        let key = self.tenant_key(target, self.config.caller_tenant, false);
        // Fast path: try cache
        if let Some(tok) = self.try_get_cached(&key).await {
            return Ok(tok);
        }
        // Cold path or near-expiry: refresh
        self.mint(&key, target, self.config.caller_tenant, false).await
    }

    async fn try_get_cached(&self, target: &str) -> Option<String> {
        let g = self.inner.try_read().ok()?;
        let tok = g.tokens.get(target).cloned()?;
        let exp = g.expires_at.get(target).copied()?;
        if exp.duration_since(Instant::now()) > self.config.refresh_skew {
            Some(tok)
        } else {
            None
        }
    }

    /// Force mint a new service-jwt for `target` and cache it.
    pub async fn refresh(&self, target: &str) -> Result<String, ServiceJwtError> {
        let key = self.tenant_key(target, self.config.caller_tenant, false);
        self.mint(&key, target, self.config.caller_tenant, false).await
    }

    /// Cache key for a per-tenant mint. The legacy fixed-tenant path keeps
    /// the plain `target` key so existing deployments are unaffected.
    fn tenant_key(&self, target: &str, tid: i64, ct: bool) -> String {
        if ct {
            format!("{target}:{tid}:ct")
        } else {
            format!("{target}:{tid}")
        }
    }

    /// Multi-tenant mint: service-jwt with `aud=target, tid=<tid>` (and
    /// `ct=true` for cross-tenant callers). Required when the calling service
    /// serves many tenants (e.g. a business API proxying file uploads on
    /// behalf of its users): the downstream tenant scope is derived from the
    /// token's `tid` claim, not from any request field. Tokens are cached per
    /// `(target, tid, ct)`; a per-tenant mint never disturbs the legacy
    /// fixed-tenant cache entry.
    pub async fn get_for_tenant(
        &self,
        target: &str,
        tid: i64,
        cross_tenant: bool,
    ) -> Result<String, ServiceJwtError> {
        let key = self.tenant_key(target, tid, cross_tenant);
        if let Some(tok) = self.try_get_cached(&key).await {
            return Ok(tok);
        }
        self.mint(&key, target, tid, cross_tenant).await
    }

    /// Mint with explicit params and store under `key`. The `params` entry is
    /// written here so the background refresh loop can re-mint without
    /// knowing how the key was created.
    async fn mint(
        &self,
        key: &str,
        target: &str,
        tid: i64,
        ct: bool,
    ) -> Result<String, ServiceJwtError> {
        // Minting flow:
        // 1. POST /tokens/service-jwt with Authorization: Bearer <bootstrap>
        // 2. Server returns access_token (RS256 JWT signed by token-service)
        //
        // For testability, we support a "test mode" where the bootstrap_token
        // is itself a pre-signed JWT — see tests for that path. In production
        // bootstrap_token is opaque and we always POST.
        let url = format!(
            "{}/tokens/service-jwt",
            self.config.token_service_url.trim_end_matches('/')
        );
        let mut body = serde_json::json!({
            "aud": target,
            "tid": tid,
        });
        if ct {
            body["ct"] = serde_json::json!(true);
        }
        let resp = self
            .http
            .post(&url)
            .timeout(self.config.request_timeout)
            .bearer_auth(&self.config.bootstrap_token)
            .json(&body)
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ServiceJwtError::AuthFailed("bootstrap rejected".into()));
        }
        let resp = resp.error_for_status()?;
        let parsed: MintResponse = resp
            .json()
            .await
            .map_err(|e| ServiceJwtError::ParseFailed(e.to_string()))?;
        let expires_at = Instant::now() + Duration::from_secs(parsed.expires_in.max(0) as u64);
        let mut g = self.inner.write().await;
        g.params
            .entry(key.to_string())
            .or_insert((target.to_string(), tid, ct));
        g.tokens.insert(key.to_string(), parsed.access_token.clone());
        g.expires_at.insert(key.to_string(), expires_at);
        Ok(parsed.access_token)
    }

    /// Pre-fill cache with a locally-signed service-jwt so subsequent
    /// `get(target)` calls return without HTTP. Test-only; production
    /// always goes through `refresh()`.
    #[doc(hidden)]
    pub async fn prime_cache(
        &self,
        target: &str,
        ttl: Duration,
    ) -> Result<String, crate::SignError> {
        let key = self.tenant_key(target, self.config.caller_tenant, false);
        let tok = self.sign_local(target, ttl)?;
        let exp = Instant::now() + ttl;
        let mut g = self.inner.write().await;
        g.params
            .entry(key.clone())
            .or_insert((target.to_string(), self.config.caller_tenant, false));
        g.tokens.insert(key.clone(), tok.clone());
        g.expires_at.insert(key, exp);
        Ok(tok)
    }

    /// Sign a local service-jwt using the same private key as token-service.
    /// Used for testing; in production, always go through the HTTP path.
    #[doc(hidden)]
    pub fn sign_local(&self, target: &str, ttl: Duration) -> Result<String, crate::SignError> {
        let now = chrono::Utc::now().timestamp();
        let claims = serde_json::json!({
            "iss": "token-service",
            "sub": self.config.caller_service_name,
            "aud": target,
            "tid": self.config.caller_tenant,
            "iat": now,
            "exp": now + ttl.as_secs() as i64,
            "roles": ["service"],
        });
        sign_token(&self.private_key_pem, &claims, ttl)
    }

    /// Background-loop refresh: re-mint by cache key using the stored params.
    async fn refresh_by_key(&self, key: &str) -> Result<String, ServiceJwtError> {
        let (target, tid, ct) = {
            let g = self.inner.read().await;
            match g.params.get(key) {
                Some(p) => p.clone(),
                // Key without params (e.g. primed via prime_cache in tests):
                // treat the whole key as the target with config tenant.
                None => (key.to_string(), self.config.caller_tenant, false),
            }
        };
        if self.try_get_cached(key).await.is_some() {
            return Ok(self.inner.read().await.tokens[key].clone());
        }
        self.mint(key, &target, tid, ct).await
    }

    pub fn spawn_refresh_loop(self: Arc<Self>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        tokio::spawn(async move {
            let interval = self.config.refresh_skew / 2;
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        let keys: Vec<String> = self.inner.try_read()
                            .map(|g| g.params.keys().cloned().collect())
                            .unwrap_or_default();
                        for k in keys {
                            if self.try_get_cached(&k).await.is_none() {
                                if let Err(e) = self.refresh_by_key(&k).await {
                                    tracing::warn!("service-jwt refresh for {} failed: {}", k, e);
                                }
                            }
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
