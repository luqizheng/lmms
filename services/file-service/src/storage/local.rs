use crate::storage::object_store::{ByteStream, ObjectStore};
use bytes::Bytes;
use digua_common_error::{AppError, AppResult};
use futures::StreamExt;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncReadExt;

pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self { base_path: base_path.into() }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        self.base_path.join(path)
    }

    /// Defense in depth: canonicalized object paths must remain below the
    /// canonical storage root. Call only after the target exists.
    async fn ensure_within_base(&self, full_path: &PathBuf) -> AppResult<()> {
        let canon_base = fs::canonicalize(&self.base_path)
            .await
            .map_err(|e| AppError::internal("FILE_STORAGE_4101", e.to_string()))?;
        let canon = fs::canonicalize(full_path)
            .await
            .map_err(|e| AppError::not_found("FILE_ITEM_1001", e.to_string()))?;
        if !canon.starts_with(&canon_base) {
            return Err(AppError::internal(
                "FILE_STORAGE_4101",
                "resolved path escapes storage root",
            ));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ObjectStore for LocalStorage {
    async fn put(&self, path: &str, data: Bytes) -> AppResult<()> {
        let full_path = self.resolve_path(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::internal("FILE_UPLOAD_2001", e.to_string()))?;
        }
        fs::write(&full_path, &data)
            .await
            .map_err(|e| AppError::internal("FILE_UPLOAD_2001", e.to_string()))?;
        if let Err(e) = self.ensure_within_base(&full_path).await {
            let _ = fs::remove_file(&full_path).await;
            return Err(e);
        }
        Ok(())
    }

    async fn get(&self, path: &str) -> AppResult<Bytes> {
        let full_path = self.resolve_path(path);
        self.ensure_within_base(&full_path).await?;
        let mut file = fs::File::open(&full_path)
            .await
            .map_err(|e| AppError::not_found("FILE_ITEM_1001", e.to_string()))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .await
            .map_err(|e| AppError::internal("FILE_READ_4001", e.to_string()))?;
        Ok(Bytes::from(buffer))
    }

    async fn get_stream(&self, path: &str) -> AppResult<ByteStream> {
        let full_path = self.resolve_path(path);
        self.ensure_within_base(&full_path).await?;
        let file = fs::File::open(&full_path)
            .await
            .map_err(|e| AppError::not_found("FILE_ITEM_1001", e.to_string()))?;
        let stream = tokio_util::io::ReaderStream::new(file);
        Ok(Box::pin(stream.map(|r| r.map_err(std::io::Error::other))))
    }

    async fn delete(&self, path: &str) -> AppResult<()> {
        let full_path = self.resolve_path(path);
        // Deletion is idempotent: after a successful object delete followed by
        // a failed metadata delete, the cleanup retry must treat a missing
        // local object as already deleted.
        if !full_path.exists() {
            return Ok(());
        }
        self.ensure_within_base(&full_path).await?;
        match fs::remove_file(&full_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::internal("FILE_DELETE_4002", error.to_string())),
        }
    }

    async fn exists(&self, path: &str) -> AppResult<bool> {
        let full_path = self.resolve_path(path);
        if !full_path.exists() {
            return Ok(false);
        }
        match self.ensure_within_base(&full_path).await {
            Ok(()) => Ok(true),
            Err(e) if e.code() == "FILE_ITEM_1001" => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn size(&self, path: &str) -> AppResult<i64> {
        let full_path = self.resolve_path(path);
        self.ensure_within_base(&full_path).await?;
        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| AppError::not_found("FILE_ITEM_1001", e.to_string()))?;
        Ok(metadata.len() as i64)
    }
}
