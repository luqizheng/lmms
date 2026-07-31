use digua_common_error::AppResult;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

#[async_trait::async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(&self, path: &str, data: Bytes) -> AppResult<()>;
    async fn get(&self, path: &str) -> AppResult<Bytes>;
    async fn get_stream(&self, path: &str) -> AppResult<ByteStream>;
    async fn delete(&self, path: &str) -> AppResult<()>;
    async fn exists(&self, path: &str) -> AppResult<bool>;
    async fn size(&self, path: &str) -> AppResult<i64>;
}
