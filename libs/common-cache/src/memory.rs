use crate::Cache;
use async_trait::async_trait;
use common_error::AppResult;
use moka::future::Cache as Moka;
use std::time::Duration;

pub struct MemoryCache {
    inner: Moka<String, Vec<u8>>,
}

impl MemoryCache {
    pub fn new(capacity: u64) -> Self {
        Self { inner: Moka::builder().max_capacity(capacity).build() }
    }
}

#[async_trait]
impl Cache for MemoryCache {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        Ok(self.inner.get(key).await)
    }
    async fn set(&self, key: &str, val: Vec<u8>, _ttl: Option<Duration>) -> AppResult<()> {
        self.inner.insert(key.to_string(), val).await; Ok(())
    }
    async fn del(&self, key: &str) -> AppResult<()> {
        self.inner.invalidate(key).await; Ok(())
    }
}