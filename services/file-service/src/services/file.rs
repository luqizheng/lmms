use crate::error::{AppError, AppResult};
use crate::models::file_info::{FileInfo, PagedFiles, QueryFiles, UploadFileMeta};
use crate::models::presign_url::{CreatePresignUrl, PresignUrl, PresignUrlResponse};
use crate::storage::ObjectStore;
use bytes::Bytes;
use common_db::DbPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct FileService {
    pool: DbPool,
    storage: Arc<dyn ObjectStore>,
    base_url: String,
}

impl FileService {
    pub fn new(pool: DbPool, storage: Arc<dyn ObjectStore>, base_url: String) -> Self {
        Self { pool, storage, base_url }
    }

    pub async fn upload(&self, data: Bytes, meta: UploadFileMeta) -> AppResult<FileInfo> {
        let id = Uuid::new_v4().to_string();
        let storage_path = format!("{}-{}", id, meta.filename);
        let now = chrono::Utc::now();
        
        self.storage.put(&storage_path, data).await?;
        
        let expires_at = meta.expires_in_seconds.map(|secs| now + chrono::Duration::seconds(secs));
        
        let info = FileInfo {
            id: id.clone(),
            filename: meta.filename,
            content_type: meta.content_type,
            size: data.len() as i64,
            hash: None,
            storage_path: storage_path.clone(),
            tenant_id: meta.tenant_id,
            user_id: meta.user_id,
            is_temporary: meta.is_temporary.unwrap_or(false),
            expires_at,
            created_at: now,
            updated_at: now,
        };
        
        self.save_file_info(&info).await?;
        
        Ok(info)
    }

    pub async fn download(&self, id: &str) -> AppResult<Bytes> {
        let info = self.get_file_info(id).await?;
        
        if let Some(expires_at) = info.expires_at {
            if expires_at < chrono::Utc::now() {
                return Err(AppError::file_not_found("File expired"));
            }
        }
        
        self.storage.get(&info.storage_path).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let info = self.get_file_info(id).await?;
        
        self.storage.delete(&info.storage_path).await?;
        self.delete_file_info(id).await?;
        
        Ok(())
    }

    pub async fn create_presign_url(&self, req: CreatePresignUrl) -> AppResult<PresignUrlResponse> {
        let info = self.get_file_info(&req.file_id).await?;
        
        let expires_in = req.expires_in_seconds.unwrap_or(3600);
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
        
        let url = format!(
            "{}{}?expires={}&token={}",
            self.base_url,
            req.file_id,
            expires_at.timestamp(),
            Uuid::new_v4()
        );
        
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

    pub async fn query_files(&self, query: QueryFiles) -> AppResult<PagedFiles> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        let files: Vec<FileInfo> = match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, FileInfo>(
                    "SELECT * FROM files ORDER BY created_at DESC LIMIT ? OFFSET ?"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, FileInfo>(
                    "SELECT * FROM files ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
        };

        let total = files.len() as i64;

        Ok(PagedFiles { items: files, total, page, size })
    }

    pub async fn get_file_info(&self, id: &str) -> AppResult<FileInfo> {
        let info: Option<FileInfo> = match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, FileInfo>("SELECT * FROM files WHERE id = ?")
                    .bind(id)
                    .fetch_optional(p)
                    .await?
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, FileInfo>("SELECT * FROM files WHERE id = ?1")
                    .bind(id)
                    .fetch_optional(p)
                    .await?
            }
        };

        info.ok_or_else(|| AppError::file_not_found(id))
    }

    async fn save_file_info(&self, info: &FileInfo) -> AppResult<()> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query(
                    r#"INSERT INTO files 
                       (id, filename, content_type, size, hash, storage_path, tenant_id, user_id, is_temporary, expires_at, created_at, updated_at)
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
                )
                .bind(&info.id)
                .bind(&info.filename)
                .bind(&info.content_type)
                .bind(info.size)
                .bind(&info.hash)
                .bind(&info.storage_path)
                .bind(&info.tenant_id)
                .bind(&info.user_id)
                .bind(info.is_temporary)
                .bind(info.expires_at)
                .bind(info.created_at)
                .bind(info.updated_at)
                .execute(p)
                .await?;
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query(
                    r#"INSERT INTO files 
                       (id, filename, content_type, size, hash, storage_path, tenant_id, user_id, is_temporary, expires_at, created_at, updated_at)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#
                )
                .bind(&info.id)
                .bind(&info.filename)
                .bind(&info.content_type)
                .bind(info.size)
                .bind(&info.hash)
                .bind(&info.storage_path)
                .bind(&info.tenant_id)
                .bind(&info.user_id)
                .bind(info.is_temporary)
                .bind(info.expires_at)
                .bind(info.created_at)
                .bind(info.updated_at)
                .execute(p)
                .await?;
            }
        }
        Ok(())
    }

    async fn delete_file_info(&self, id: &str) -> AppResult<()> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query("DELETE FROM files WHERE id = ?")
                    .bind(id)
                    .execute(p)
                    .await?;
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query("DELETE FROM files WHERE id = ?1")
                    .bind(id)
                    .execute(p)
                    .await?;
            }
        }
        Ok(())
    }

    async fn save_presign_url(&self, presign: &PresignUrl) -> AppResult<()> {
        match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query(
                    r#"INSERT INTO presign_urls 
                       (id, file_id, operation, url, expires_at, created_by, created_at)
                       VALUES (?, ?, ?, ?, ?, ?, ?)"#
                )
                .bind(&presign.id)
                .bind(&presign.file_id)
                .bind(&presign.operation)
                .bind(&presign.url)
                .bind(presign.expires_at)
                .bind(&presign.created_by)
                .bind(presign.created_at)
                .execute(p)
                .await?;
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query(
                    r#"INSERT INTO presign_urls 
                       (id, file_id, operation, url, expires_at, created_by, created_at)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#
                )
                .bind(&presign.id)
                .bind(&presign.file_id)
                .bind(&presign.operation)
                .bind(&presign.url)
                .bind(presign.expires_at)
                .bind(&presign.created_by)
                .bind(presign.created_at)
                .execute(p)
                .await?;
            }
        }
        Ok(())
    }
}