use crate::error::{AppError, AppErrorExt, AppResult};
use crate::models::file_info::{FileInfo, PagedFiles, QueryFiles, UploadFileMeta};
use crate::models::presign_url::{CreatePresignUrl, PresignUrl, PresignUrlResponse};
use crate::storage::object_store::ObjectStore;
use bytes::Bytes;
use digua_common_db::WithTenantScope;
use digua_common_db::{db_execute, db_qb_execute, db_qb_fetch_all, db_qb_fetch_optional, DbPool};
use digua_jwt_ctx::TenantScope;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct FileService {
    pool: DbPool,
    storage: Arc<dyn ObjectStore>,
    base_url: String,
    max_file_size: u64,
    /// Phase 5-B: JWKS cache for verifying inbound user-jwt.
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
        Self {
            pool,
            storage,
            base_url,
            max_file_size,
            jwks,
        }
    }

    pub fn pool_ref(&self) -> &DbPool {
        &self.pool
    }

    pub async fn upload(
        &self,
        data: Bytes,
        meta: UploadFileMeta,
        scope: &TenantScope,
    ) -> AppResult<FileInfo> {
        if (data.len() as u64) > self.max_file_size {
            return Err(AppError::file_too_large(format!(
                "file too large: {} > {} bytes",
                data.len(),
                self.max_file_size
            )));
        }
        let id = Uuid::new_v4().to_string();
        // 2026-11-05 review (P0-5): the object key must contain zero
        // client-controlled bytes. It used to be `{uuid}-{meta.filename}`,
        // and `LocalStorage::resolve_path` does a plain `PathBuf::join`, so a
        // filename like `x/../../../../tmp/pwn.sh` escaped the storage root
        // (write) and the same path in DB read back arbitrary files
        // (download). The user-visible name lives in `FileInfo.filename`
        // only.
        let storage_path = id.clone();
        let now = chrono::Utc::now();
        let data_len = data.len();
        let tenant_id = scope.tenant_id().unwrap_or(1);

        let expires_at = meta
            .expires_in_seconds
            .map(|secs| now + chrono::Duration::seconds(secs));

        let info = FileInfo {
            url: String::new(),
            id: id.clone(),
            filename: meta.filename,
            content_type: meta.content_type,
            size: data_len as i64,
            hash: None,
            storage_path: storage_path.clone(),
            tenant_id,
            user_id: meta.user_id,
            is_temporary: meta.is_temporary.unwrap_or(false),
            expires_at,
            created_at: now,
            updated_at: now,
        };

        ObjectStore::put(&*self.storage, &storage_path, data).await?;
        self.save_file_info(&info).await?;

        Ok(info)
    }

    // check-tenant-scope: delegated (and_tenant applied inside get_file_info)
    pub async fn download(&self, id: &str, scope: &TenantScope) -> AppResult<Bytes> {
        let info = self.get_file_info(id, scope).await?;

        if let Some(expires_at) = info.expires_at {
            if expires_at < chrono::Utc::now() {
                return Err(AppError::file_not_found("File expired"));
            }
        }

        ObjectStore::get(&*self.storage, &info.storage_path).await
    }

    // check-tenant-scope: delegated (and_tenant applied inside delete_file_info)
    pub async fn delete(&self, id: &str, scope: &TenantScope) -> AppResult<()> {
        let info = self.get_file_info(id, scope).await?;

        ObjectStore::delete(&*self.storage, &info.storage_path).await?;
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
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);

        let url = match req.operation.as_str() {
            "put" => {
                self.storage
                    .presign_put(&info.storage_path, expires_in.max(0) as u64)
                    .await?
            }
            _ => {
                self.storage
                    .presign_get(&info.storage_path, expires_in.max(0) as u64)
                    .await?
            }
        }
        .unwrap_or_else(|| {
            format!(
                "{}{}?expires={}&token={}",
                self.base_url,
                req.file_id,
                expires_at.timestamp(),
                Uuid::new_v4()
            )
        });

        let presign = PresignUrl {
            id: Uuid::new_v4().to_string(),
            file_id: req.file_id.clone(),
            operation: req.operation.clone(),
            url: url.clone(),
            expires_at,
            created_by: None,
            created_at: chrono::Utc::now(),
        };

        self.save_presign_url(&presign).await?;

        Ok(PresignUrlResponse {
            url,
            expires_at,
            operation: req.operation,
        })
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
            qb.push(" ORDER BY created_at DESC LIMIT ")
                .push_bind(size as i64);
            qb.push(" OFFSET ").push_bind(offset as i64);
        })?;
        let total = files.len() as i64;

        Ok(PagedFiles {
            items: files,
            total,
            page,
            size,
        })
    }

    pub async fn get_file_info(&self, id: &str, scope: &TenantScope) -> AppResult<FileInfo> {
        let info: Option<FileInfo> = db_qb_fetch_optional!(&self.pool, FileInfo, |qb| {
            qb.push("SELECT * FROM file_files");
            qb.where_id(id);
            qb.and_tenant(scope);
        })?;

        info.ok_or_else(|| AppError::file_not_found(id))
    }

    async fn save_file_info(&self, info: &FileInfo) -> AppResult<()> {
        db_execute!(
            &self.pool,
            r#"INSERT INTO file_files
               (id, filename, content_type, size, hash, storage_path, tenant_id, user_id, is_temporary, expires_at, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            &info.id,
            &info.filename,
            &info.content_type,
            info.size,
            &info.hash,
            &info.storage_path,
            info.tenant_id,
            &info.user_id,
            info.is_temporary,
            info.expires_at,
            info.created_at,
            info.updated_at
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
            &presign.id,
            &presign.file_id,
            &presign.operation,
            &presign.url,
            presign.expires_at,
            &presign.created_by,
            presign.created_at
        )?;
        Ok(())
    }
}
