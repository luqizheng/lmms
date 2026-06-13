pub mod memory;
pub use memory::MemoryCache;

use async_trait::async_trait;
use common_error::AppResult;
use std::time::Duration;

#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>>;
    async fn set(&self, key: &str, val: Vec<u8>, ttl: Option<Duration>) -> AppResult<()>;
    async fn del(&self, key: &str) -> AppResult<()>;
}