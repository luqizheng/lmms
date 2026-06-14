pub mod memory;
#[cfg(feature = "redis")]
pub mod redis;

pub use memory::MemoryCache;
#[cfg(feature = "redis")]
pub use redis::{DistributedLock, RedisCache};

use async_trait::async_trait;
use common_error::AppResult;
use std::time::Duration;

#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>>;
    async fn set(&self, key: &str, val: Vec<u8>, ttl: Option<Duration>) -> AppResult<()>;
    async fn del(&self, key: &str) -> AppResult<()>;
    async fn incr(&self, key: &str) -> AppResult<i64> {
        let _ = key;
        Err(common_error::AppError::internal("CACHE_NOT_SUPPORTED", "incr not supported"))
    }
    async fn mget(&self, keys: &[&str]) -> AppResult<Vec<Option<Vec<u8>>>> {
        let _ = keys;
        Err(common_error::AppError::internal("CACHE_NOT_SUPPORTED", "mget not supported"))
    }
}

pub struct TieredCache {
    local: MemoryCache,
    remote: Box<dyn Cache>,
}

impl TieredCache {
    pub fn new(local_capacity: u64, remote: Box<dyn Cache>) -> Self {
        Self {
            local: MemoryCache::new(local_capacity),
            remote,
        }
    }
}

#[async_trait]
impl Cache for TieredCache {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        if let Some(val) = self.local.get(key).await? {
            return Ok(Some(val));
        }
        
        if let Some(val) = self.remote.get(key).await? {
            self.local.set(key, val.clone(), None).await?;
            return Ok(Some(val));
        }
        
        Ok(None)
    }

    async fn set(&self, key: &str, val: Vec<u8>, ttl: Option<Duration>) -> AppResult<()> {
        self.local.set(key, val.clone(), ttl).await?;
        self.remote.set(key, val, ttl).await?;
        Ok(())
    }

    async fn del(&self, key: &str) -> AppResult<()> {
        self.local.del(key).await?;
        self.remote.del(key).await?;
        Ok(())
    }
}