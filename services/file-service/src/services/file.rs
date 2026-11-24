use crate::error::{AppError, AppErrorExt, AppResult};
use crate::models::file_info::{FileInfo, PagedFiles, QueryFiles, UploadFileMeta};
use crate::models::presign_url::{CreatePresignUrl, PresignOperation, PresignUrl, PresignUrlResponse};
use crate::storage::object_store::ObjectStore;
use bytes::Bytes;
use digua_common_db::WithTenantScope;
use digua_common_db::{db_execute, db_qb_execute, db_qb_fetch_all, db_qb_fetch_optional, DbPool};
use digua_jwt_ctx::TenantScope;
use std::sync::Arc;
use uuid::Uuid;

/// S3 SigV4 query authentication supports at most seven days.
const MAX_PRESIGN_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const DEFAULT_TEMPORARY_TTL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Clone)]
pub struct FileService {
    pool: DbPool,
    storage: Arc<dyn ObjectStore>,
    base_url: String,
    max_file_size: u64,
    /// JWKS cache used by the service-JWT audience middleware.
    pub jwks: Arc<digua_common_jwt::JWKSCache>,
}

impl FileService {
    pub fn new(
        pool: DbPool,
        storage: Arc<dyn ObjectStore>,
        base_url: String,
        max_file_size: u64,
        jwks: Arc<digua_common_jwt::JWKSCache>,
    ) -> Self {
        Self { pool, storage, base_url, max_file_size, jwks }
    }

    pub fn pool_ref(&self) -> &DbPool { &self.pool }

    pub async fn upload(
        &self,
        data: Bytes,
        meta: UploadFileMeta,
        scope: &TenantScope,
    ) -> AppResult<FileInfo> {
        if (data.len() as u64) > self.max_file_size {
            return Err(AppError::file_too_large(format!(
                "file too large: {} > {} bytes", data.len(), self.max_file_size
            )));
        }

        let is_temporary = meta.is_temporary.unwrap_or(false);
        let now = chrono::Utc::now();
        let expires_at = if is_temporary {
            let ttl = meta.expires_in_seconds.unwrap_or(DEFAULT_TEMPORARY_TTL_SECONDS);
            if ttl <= 0 {
                return Err(AppError::bad_request("FILE_UPLOAD_2003", "temporary file TTL must be positive"));
            }
            Some(now + chrono::Duration::seconds(ttl))
        } else {
            None
        };

        let id = Uuid::new_v4().to_string();
        // Object keys are server-generated and never contain client-controlled filename bytes.
        let storage_path = id.clone();
        let data_len = data.len();
        let info = FileInfo {
            url: String::new(),
            id: id.clone(),
            filename: meta.filename,
            content_type: meta.content_type,
            size: data_len as i64,
            hash: None,
            storage_path: storage_path.clone(),
            tenant_id: scope.tenant_id().unwrap_or(1),
            user_id: meta.user_id,
            is_temporary,
            expires_at,
            created_at: now,
            updated_at: now,
        };

        self.storage.put(&storage_path, data).await?;
        if let Err(db_error) = self.save_file_info(&info).await {
            // Compensate object write if metadata persistence fails. A failed
            // compensation is logged for the periodic orphan reconciliation work.
            if let Err(cleanup_error) = self.storage.delete(&storage_path).await {
                tracing::error!(
                    file_id = %id,
                    error = %cleanup_error,
                    "failed to compensate object after metadata insert failure"
                );
            }
            return Err(db_error);
        }

        Ok(info)
    }

    // check-tenant-scope: delegated (and_tenant applied inside get_file_info)
    pub async fn download(&self, id: &str, scope: &TenantScope) -> AppResult<Bytes> {
        let info = self.get_file_info(id, scope).await?;
        if let Some(expires_at) = info.expires_at {
            if expires_at <= chrono::Utc::now() {
                return Err(AppError::file_not_found("File expired"));
            }
        }
        self.storage.get(&info.storage_path).await
    }

    // Object deletion precedes metadata deletion. If DB deletion fails, a retry
    // can safely repeat the idempotent object delete and then remove the row.
    // check-tenant-scope: delegated (and_tenant applied inside get_file_info/delete_file_info)
    pub async fn delete(&self, id: &str, scope: &TenantScope) -> AppResult<()> {
        let info = self.get_file_info(id, scope).await?;
        // Presign rows reference file_files through a foreign key. Revoke them
        // before deleting the object and its metadata.
        db_execute!(&self.pool, "DELETE FROM file_presign_urls WHERE file_id = ?", id)?;
        self.storage.delete(&info.storage_path).await?;
        self.delete_file_info(id, scope).await?;
        Ok(())
    }

    // check-tenant-scope: delegated (and_tenant applied inside get_file_info)
    pub async fn create_presign_url(
        &self,
        req: CreatePresignUrl,
        scope: &TenantScope,
    ) -> AppResult<PresignUrlResponse> {
        let info = self.get_file_info(&req.file_id, scope).await?;
        let expires_in = req.expires_in_seconds.unwrap_or(3600);
        if !(1..=MAX_PRESIGN_TTL_SECONDS).contains(&expires_in) {
            return Err(AppError::bad_request(
                "FILE_PRESIGN_3002",
                format!("presign TTL must be between 1 and {MAX_PRESIGN_TTL_SECONDS} seconds"),
            ));
        }
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);

        let url = match req.operation {
            PresignOperation::Put => self.storage.presign_put(&info.storage_path, expires_in as u64).await?,
            PresignOperation::Get => self.storage.presign_get(&info.storage_path, expires_in as u64).await?,
        }
        .unwrap_or_else(|| {
            format!(
                "{}{}?expires={}&token={}",
                self.base_url, req.file_id, expires_at.timestamp(), Uuid::new_v4()
            )
        });

        let operation = req.operation;
        let presign = PresignUrl {
            id: Uuid::new_v4().to_string(),
            file_id: req.file_id.clone(),
            operation: operation.as_str().to_string(),
            url: url.clone(),
            expires_at,
            created_by: None,
            created_at: chrono::Utc::now(),
        };
        self.save_presign_url(&presign).await?;

        Ok(PresignUrlResponse { url, expires_at, operation })
    }

    pub async fn query_files(
        &self,
        query: QueryFiles,
        scope: &TenantScope,
    ) -> AppResult<PagedFiles> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;
        let files: Vec<FileInfo> = db_qb_fetch_all!(&self.pool, FileInfo, |qb| {
            qb.push("SELECT * FROM file_files WHERE 1=1");
            qb.and_tenant(scope);
            qb.push(" ORDER BY created_at DESC LIMIT ").push_bind(size as i64);
            qb.push(" OFFSET ").push_bind(offset as i64);
        })?;
        let total = files.len() as i64;
        Ok(PagedFiles { items: files, total, page, size })
    }

    pub async fn get_file_info(&self, id: &str, scope: &TenantScope) -> AppResult<FileInfo> {
        let info: Option<FileInfo> = db_qb_fetch_optional!(&self.pool, FileInfo, |qb| {
            qb.push("SELECT * FROM file_files");
            qb.where_id(id);
            qb.and_tenant(scope);
        })?;
        info.ok_or_else(|| AppError::file_not_found(id))
    }

    /// Delete expired temporary objects in bounded batches.
    // check-tenant-scope: internal system maintenance intentionally spans tenants.
    /// Failed object deletes retain their metadata row and are retried next cycle.
    pub async fn cleanup_expired(&self, limit: i64) -> AppResult<usize> {
        let now = chrono::Utc::now();
        // Expired capabilities are no longer usable; remove them to avoid
        // retaining stale bearer URLs and to release file metadata references.
        db_execute!(&self.pool, "DELETE FROM file_presign_urls WHERE expires_at <= ?", now)?;
        let expired: Vec<FileInfo> = db_qb_fetch_all!(&self.pool, FileInfo, |qb| {
            qb.push("SELECT * FROM file_files WHERE is_temporary = ");
            qb.push_bind(true);
            qb.push(" AND expires_at IS NOT NULL AND expires_at <= ");
            qb.push_bind(now);
            qb.push(" ORDER BY expires_at ASC LIMIT ").push_bind(limit.max(1));
        })?;

        let mut removed = 0usize;
        for info in expired {
            db_execute!(&self.pool, "DELETE FROM file_presign_urls WHERE file_id = ?", &info.id)?;
            if let Err(error) = self.storage.delete(&info.storage_path).await {
                tracing::warn!(file_id = %info.id, error = %error, "expired file object deletion failed; will retry");
                continue;
            }
            // If this fails, the next cycle retries the idempotent object delete
            // and then attempts metadata removal again.
            db_execute!(&self.pool, "DELETE FROM file_files WHERE id = ?", &info.id)?;
            removed += 1;
        }
        Ok(removed)
    }

    async fn save_file_info(&self, info: &FileInfo) -> AppResult<()> {
        db_execute!(
            &self.pool,
            r#"INSERT INTO file_files
               (id, filename, content_type, size, hash, storage_path, tenant_id, user_id, is_temporary, expires_at, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            &info.id, &info.filename, &info.content_type, info.size, &info.hash,
            &info.storage_path, info.tenant_id, &info.user_id, info.is_temporary,
            info.expires_at, info.created_at, info.updated_at
        )?;
        Ok(())
    }

    async fn delete_file_info(&self, id: &str, scope: &TenantScope) -> AppResult<()> {
        db_qb_execute!(&self.pool, |qb| {
            qb.push("DELETE FROM file_files");
            qb.where_id(id);
            qb.and_tenant(scope);
        })?;
        Ok(())
    }

    async fn save_presign_url(&self, presign: &PresignUrl) -> AppResult<()> {
        db_execute!(
            &self.pool,
            r#"INSERT INTO file_presign_urls
               (id, file_id, operation, url, expires_at, created_by, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
            &presign.id, &presign.file_id, &presign.operation, &presign.url,
            presign.expires_at, &presign.created_by, presign.created_at
        )?;
        Ok(())
    }
}
