use crate::error::AppErrorExt;
use crate::storage::object_store::{ByteStream, ObjectStore};
use bytes::Bytes;
use digua_common_error::{AppError, AppResult};
use futures::StreamExt;
use s3::creds::Credentials;
use s3::region::Region;
use s3::{Bucket, BucketConfiguration};
use std::sync::Arc;

/// S3 协议 driver —— 同源代码兼容 RustFS / MinIO / AWS S3 / Aliyun OSS / 腾讯 COS 等 S3-compatible 后端。
///
/// driver 之间的差异全部体现在配置参数（endpoint / path_style / region），不增加代码分支：
/// - **RustFS / MinIO / 自建**: path-style = true, endpoint = rustfs url
/// - **AWS S3**:           path-style = false, region = us-east-1 等
/// - **Aliyun OSS**:       path-style = false, region = oss-cn-hangzhou, 自定义 endpoint
///
/// `FILE_STORAGE_BACKEND=rustfs|s3` 仅在 `main.rs` 决定加载哪个 `S3Config`，构造本类型时 driver 已扁平化。
pub struct S3Storage {
    bucket: Arc<Bucket>,
    bucket_name: String,
}

#[derive(Clone, Debug)]
pub struct S3Config {
    pub bucket_name: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    /// true = path-style (`http://endpoint/bucket/key`), false = vhost (`http://bucket.endpoint/key`)。
    /// RustFS / MinIO / 自建必须 true;AWS / Aliyun 默认 false。
    pub path_style: bool,
    /// 启动时 attempt 创建 bucket。需要 `s3:CreateBucket` 权限;失败 warn 但不阻断启动。
    pub auto_create_bucket: bool,
}

impl S3Storage {
    pub fn new(config: S3Config) -> AppResult<Self> {
        if config.bucket_name.trim().is_empty() {
            return Err(AppError::bad_request(
                "FILE_STORAGE_5001",
                "s3 bucket name is empty",
            ));
        }
        if config.access_key.trim().is_empty() || config.secret_key.trim().is_empty() {
            return Err(AppError::bad_request(
                "FILE_STORAGE_5002",
                "s3 access_key / secret_key is empty",
            ));
        }

        let region = match &config.endpoint {
            Some(ep) if !ep.trim().is_empty() => Region::Custom {
                region: config.region.clone(),
                endpoint: ep.trim_end_matches('/').to_string(),
            },
            _ => Region::Custom {
                region: config.region.clone(),
                endpoint: format!("https://s3.{}.amazonaws.com", config.region),
            },
        };

        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| AppError::internal("FILE_STORAGE_5003", format!("build credentials: {e}")))?;

        let raw_bucket: Box<Bucket> =
            Bucket::new(&config.bucket_name, region.clone(), credentials.clone()).map_err(|e| {
                AppError::internal("FILE_STORAGE_5004", format!("create bucket handle: {e}"))
            })?;
        let bucket: Bucket = match config.path_style {
            true => *raw_bucket.with_path_style(),
            false => *raw_bucket,
        };
        let bucket = Arc::new(bucket);

        let s = Self {
            bucket,
            bucket_name: config.bucket_name.clone(),
        };
        if config.auto_create_bucket {
            // ensure_bucket 不阻塞启动,失败记 warn(运维可预先建好)。
            let region_for_create = region.clone();
            let creds_for_create = credentials.clone();
            let bucket_for_check = s.bucket.clone();
            let name = s.bucket_name.clone();
            let path_style = config.path_style;
            tokio::spawn(async move {
                if let Err(e) = ensure_bucket(
                    bucket_for_check,
                    name,
                    region_for_create,
                    creds_for_create,
                    path_style,
                )
                .await
                {
                    tracing::warn!("ensure_bucket failed (non-fatal): {e}");
                }
            });
        }
        Ok(s)
    }

    pub fn bucket_name(&self) -> &str {
        &self.bucket_name
    }
}

#[allow(clippy::too_many_arguments)]
async fn ensure_bucket(
    bucket: Arc<Bucket>,
    name: String,
    region: Region,
    creds: Credentials,
    path_style: bool,
) -> AppResult<()> {
    match bucket.exists().await {
        Ok(true) => {
            tracing::debug!("s3 bucket '{}' already exists", name);
            return Ok(());
        }
        Ok(false) => {
            tracing::info!("s3 bucket '{}' does not exist, attempting create", name);
        }
        Err(e) => {
            tracing::warn!(
                "s3 bucket '{}' exists-check failed: {e} (falling through to create)",
                name
            );
        }
    }

    let cfg = BucketConfiguration::default();
    let create_result = if path_style {
        Bucket::create_with_path_style(&name, region, creds, cfg).await
    } else {
        Bucket::create(&name, region, creds, cfg).await
    };
    match create_result {
        Ok(resp) if resp.response_code < 400 => {
            tracing::info!(
                "s3 bucket '{}' created (status={})",
                name,
                resp.response_code
            );
            Ok(())
        }
        Ok(resp) => Err(AppError::internal(
            "FILE_STORAGE_5005",
            format!(
                "create_bucket '{}' returned status={} body={}",
                name, resp.response_code, resp.response_text
            ),
        )),
        Err(e) => Err(AppError::internal(
            "FILE_STORAGE_5005",
            format!("create_bucket '{}': {e}", name),
        )),
    }
}

fn map_s3_err(context: &str, e: impl std::fmt::Display) -> AppError {
    AppError::internal("FILE_STORAGE_5000", format!("{context}: {e}"))
}

#[async_trait::async_trait]
impl ObjectStore for S3Storage {
    async fn put(&self, path: &str, data: Bytes) -> AppResult<()> {
        let resp = self
            .bucket
            .put_object(path, &data)
            .await
            .map_err(|e| map_s3_err("s3 put_object", e))?;
        let code = resp.status_code();
        if code >= 300 {
            let body = String::from_utf8_lossy(resp.as_slice()).into_owned();
            return Err(AppError::internal(
                "FILE_STORAGE_5010",
                format!("put_object status={code}: {body}"),
            ));
        }
        Ok(())
    }

    async fn get(&self, path: &str) -> AppResult<Bytes> {
        let resp = self
            .bucket
            .get_object(path)
            .await
            .map_err(|e| map_s3_err("s3 get_object", e))?;
        let code = resp.status_code();
        if code == 404 {
            return Err(AppError::file_not_found(format!(
                "s3 key not found: {path}"
            )));
        }
        if code >= 300 {
            let body = String::from_utf8_lossy(resp.as_slice()).into_owned();
            return Err(AppError::internal(
                "FILE_STORAGE_5011",
                format!("get_object status={code}: {body}"),
            ));
        }
        Ok(Bytes::from(resp.to_vec()))
    }

    async fn get_stream(&self, path: &str) -> AppResult<ByteStream> {
        let mut stream = self
            .bucket
            .get_object_stream(path)
            .await
            .map_err(|e| map_s3_err("s3 get_object_stream", e))?;
        if stream.status_code == 404 {
            return Err(AppError::file_not_found(format!(
                "s3 key not found: {path}"
            )));
        }
        if stream.status_code >= 300 {
            return Err(AppError::internal(
                "FILE_STORAGE_5011",
                format!("get_object_stream status={}", stream.status_code),
            ));
        }
        let inner = std::mem::replace(&mut stream.bytes, Box::pin(futures::stream::empty()));
        let mapped = inner.map(|item| match item {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(std::io::Error::other(e)),
        });
        Ok(Box::pin(mapped))
    }

    async fn delete(&self, path: &str) -> AppResult<()> {
        let resp = self
            .bucket
            .delete_object(path)
            .await
            .map_err(|e| map_s3_err("s3 delete_object", e))?;
        let code = resp.status_code();
        if code >= 300 && code != 404 {
            let body = String::from_utf8_lossy(resp.as_slice()).into_owned();
            return Err(AppError::internal(
                "FILE_STORAGE_5012",
                format!("delete_object status={code}: {body}"),
            ));
        }
        Ok(())
    }

    async fn exists(&self, path: &str) -> AppResult<bool> {
        self.bucket
            .object_exists(path)
            .await
            .map_err(|e| map_s3_err("s3 object_exists", e))
    }

    async fn size(&self, path: &str) -> AppResult<i64> {
        let (head, status) = self
            .bucket
            .head_object(path)
            .await
            .map_err(|e| map_s3_err("s3 head_object", e))?;
        if status == 404 {
            return Err(AppError::file_not_found(format!(
                "s3 key not found: {path}"
            )));
        }
        head.content_length.ok_or_else(|| {
            AppError::internal(
                "FILE_STORAGE_5013",
                "head_object missing Content-Length header",
            )
        })
    }

    async fn presign_get(&self, path: &str, ttl_seconds: u64) -> AppResult<Option<String>> {
        let ttl = ttl_to_u32(ttl_seconds)?;
        let url = self
            .bucket
            .presign_get(path, ttl, None)
            .await
            .map_err(|e| map_s3_err("s3 presign_get", e))?;
        Ok(Some(url))
    }

    async fn presign_put(&self, path: &str, ttl_seconds: u64) -> AppResult<Option<String>> {
        let ttl = ttl_to_u32(ttl_seconds)?;
        let url = self
            .bucket
            .presign_put(path, ttl, None, None)
            .await
            .map_err(|e| map_s3_err("s3 presign_put", e))?;
        Ok(Some(url))
    }
}

fn ttl_to_u32(ttl_seconds: u64) -> AppResult<u32> {
    u32::try_from(ttl_seconds).map_err(|_| {
        AppError::bad_request(
            "FILE_STORAGE_5014",
            "presign ttl exceeds u32 (max ~136 years)",
        )
    })
}
