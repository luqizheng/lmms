use bytes::Bytes;
use digua_common_error::AppResult;
use futures::Stream;
use std::pin::Pin;

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

/// 上传/下载对象存储的统一抽象。
///
/// 私有化部署有 3 类 driver：
/// - `LocalStorage` — 文件系统，dev 默认
/// - `S3Storage`    — S3 协议（RustFS / AWS S3 / 阿里 OSS / MinIO 共用一份实现）
///
/// `presign_get` / `presign_put` 默认返回 `Ok(None)`，表示该 driver 不支持原生
/// 预签名 URL。FileService 会明确返回 501，不会生成无法验证的伪签名代理 URL。
#[async_trait::async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(&self, path: &str, data: Bytes) -> AppResult<()>;
    async fn get(&self, path: &str) -> AppResult<Bytes>;
    async fn get_stream(&self, path: &str) -> AppResult<ByteStream>;
    async fn delete(&self, path: &str) -> AppResult<()>;
    async fn exists(&self, path: &str) -> AppResult<bool>;
    async fn size(&self, path: &str) -> AppResult<i64>;

    /// 生成 GET 预签名 URL。`Ok(None)` 表示 driver 不支持；调用方必须 fail closed。
    async fn presign_get(&self, _path: &str, _ttl_seconds: u64) -> AppResult<Option<String>> {
        Ok(None)
    }

    /// 生成 PUT 预签名 URL（用于客户端直传）。`Ok(None)` 表示不支持。
    async fn presign_put(&self, _path: &str, _ttl_seconds: u64) -> AppResult<Option<String>> {
        Ok(None)
    }
}
