use crate::models::TokenInfo;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct CachedToken {
    info: TokenInfo,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct CacheKey {
    client_id: String,
    secret_key: String,
}

impl CacheKey {
    fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.client_id.as_bytes());
        h.update(b"|");
        h.update(self.secret_key.as_bytes());
        hex::encode(h.finalize())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TokenCacheSnapshot {
    pub size: usize,
}

#[derive(Clone)]
pub struct ServiceTokenCache {
    refresh_skew: Duration,
    inner: Arc<Mutex<HashMap<String, (CacheKey, CachedToken)>>>,
}

impl ServiceTokenCache {
    pub fn new(refresh_skew: Duration) -> Self {
        Self {
            refresh_skew,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn snapshot(&self) -> TokenCacheSnapshot {
        TokenCacheSnapshot {
            size: self.inner.lock().await.len(),
        }
    }

    pub async fn get_or_fetch<F, Fut>(
        &self,
        client_id: &str,
        secret_key: &str,
        fetcher: F,
    ) -> Result<String, crate::error::KudiHttpError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<TokenInfo, crate::error::KudiHttpError>>,
    {
        let key = CacheKey {
            client_id: client_id.to_string(),
            secret_key: secret_key.to_string(),
        };
        let digest = key.digest();

        {
            let guard = self.inner.lock().await;
            if let Some((stored_key, cached)) = guard.get(&digest) {
                if stored_key.client_id == client_id
                    && stored_key.secret_key == secret_key
                    && Instant::now() < cached.expires_at - self.refresh_skew
                {
                    return Ok(cached.info.access_token.clone());
                }
            }
        }

        let mut guard = self.inner.lock().await;
        if let Some((stored_key, cached)) = guard.get(&digest) {
            if stored_key.client_id == client_id
                && stored_key.secret_key == secret_key
                && Instant::now() < cached.expires_at - self.refresh_skew
            {
                return Ok(cached.info.access_token.clone());
            }
        }

        let info = fetcher().await?;
        let expires_at = Instant::now() + Duration::from_secs(info.expires_in.max(0) as u64);
        guard.insert(
            digest,
            (
                key,
                CachedToken {
                    info: info.clone(),
                    expires_at,
                },
            ),
        );
        Ok(info.access_token)
    }

    pub async fn invalidate(&self, client_id: &str, secret_key: &str) {
        let digest = CacheKey {
            client_id: client_id.to_string(),
            secret_key: secret_key.to_string(),
        }
        .digest();
        self.inner.lock().await.remove(&digest);
    }
}
