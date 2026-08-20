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
        Self {
            base_path: base_path.into(),
        }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        self.base_path.join(path)
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

        Ok(())
    }

    async fn get(&self, path: &str) -> AppResult<Bytes> {
        let full_path = self.resolve_path(path);

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

        let file = fs::File::open(&full_path)
            .await
            .map_err(|e| AppError::not_found("FILE_ITEM_1001", e.to_string()))?;

        let stream = tokio_util::io::ReaderStream::new(file);
        Ok(Box::pin(stream.map(|r| {
            r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        })))
    }

    async fn delete(&self, path: &str) -> AppResult<()> {
        let full_path = self.resolve_path(path);

        if full_path.exists() {
            fs::remove_file(&full_path)
                .await
                .map_err(|e| AppError::internal("FILE_DELETE_4002", e.to_string()))?;
        }

        Ok(())
    }

    async fn exists(&self, path: &str) -> AppResult<bool> {
        let full_path = self.resolve_path(path);
        Ok(full_path.exists())
    }

    async fn size(&self, path: &str) -> AppResult<i64> {
        let full_path = self.resolve_path(path);

        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| AppError::not_found("FILE_ITEM_1001", e.to_string()))?;

        Ok(metadata.len() as i64)
    }
}
