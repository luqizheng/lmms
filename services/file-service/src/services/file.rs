use crate::error::{AppError, AppErrorExt, AppResult};
use crate::models::file_info::{FileInfo, PagedFiles, QueryFiles, UploadFileMeta};
use crate::models::presign_url::{CreatePresignUrl, PresignUrl, PresignUrlResponse};
use crate::storage::object_store::ObjectStore;
use bytes::Bytes;
use digua_common_db::WithTenantScope;
use digua_jwt_ctx::TenantScope;
use sqlx::{MySqlPool, QueryBuilder};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct FileService {
    pool: MySqlPool,
    storage: Arc<dyn ObjectStore>,
    base_url: String,
    max_file_size: u64,
    /// Phase 5-B: JWKS cache for verifying inbound user-jwt.
    pub jwks: Arc<digua_common_jwt::JWKSCache>,
}

impl FileService {
    pub fn new(
        pool: MySqlPool,
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

    pub fn pool_ref(&self) -> &MySqlPool {
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
        let storage_path = format!("{}-{}", id, meta.filename);
        let now = chrono::Utc::now();
        let data_len = data.len();
        let tenant_id = scope.tenant_id().unwrap_or(1) as u64;

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

        let mut qb = QueryBuilder::new("SELECT * FROM file_files WHERE 1=1");
        qb.and_tenant(scope);
        qb.push(" ORDER BY created_at DESC LIMIT ");
        qb.push_bind(size);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let files: Vec<FileInfo> = qb
            .build_query_as::<FileInfo>()
            .fetch_all(&self.pool)
            .await?;
        let total = files.len() as i64;

        Ok(PagedFiles {
            items: files,
            total,
            page,
            size,
        })
    }

    pub async fn get_file_info(&self, id: &str, scope: &TenantScope) -> AppResult<FileInfo> {
        let mut qb = QueryBuilder::new("SELECT * FROM file_files");
        qb.where_id(id);
        qb.and_tenant(scope);

        let info: Option<FileInfo> = qb
            .build_query_as::<FileInfo>()
            .fetch_optional(&self.pool)
            .await?;

        info.ok_or_else(|| AppError::file_not_found(id))
    }

    async fn save_file_info(&self, info: &FileInfo) -> AppResult<()> {
        sqlx::query(
            r#"INSERT INTO file_files
               (id, filename, content_type, size, hash, storage_path, tenant_id, user_id, is_temporary, expires_at, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&info.id)
        .bind(&info.filename)
        .bind(&info.content_type)
        .bind(info.size)
        .bind(&info.hash)
        .bind(&info.storage_path)
        .bind(info.tenant_id)
        .bind(&info.user_id)
        .bind(info.is_temporary)
        .bind(info.expires_at)
        .bind(info.created_at)
        .bind(info.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_file_info(&self, id: &str, scope: &TenantScope) -> AppResult<()> {
        let mut qb = QueryBuilder::new("DELETE FROM file_files");
        qb.where_id(id);
        qb.and_tenant(scope);
        qb.build().execute(&self.pool).await?;
        Ok(())
    }

    async fn save_presign_url(&self, presign: &PresignUrl) -> AppResult<()> {
        sqlx::query(
            r#"INSERT INTO file_presign_urls
               (id, file_id, operation, url, expires_at, created_by, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&presign.id)
        .bind(&presign.file_id)
        .bind(&presign.operation)
        .bind(&presign.url)
        .bind(presign.expires_at)
        .bind(&presign.created_by)
        .bind(presign.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
