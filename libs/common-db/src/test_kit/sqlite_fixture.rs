use common_error::{AppError, AppResult};
use sqlx::SqlitePool;
use std::path::Path;
use tempfile::NamedTempFile;

pub struct SqliteFixture {
    _file: NamedTempFile,
    pool: SqlitePool,
    pub path: String,
}

impl SqliteFixture {
    pub async fn new(migration_dir: &str) -> AppResult<Self> {
        let file = NamedTempFile::new().map_err(|e| AppError::internal("FX_TMP", e.to_string()))?;
        let path = file.path().to_string_lossy().to_string();
        let url = format!("sqlite://{}?mode=rwc", path);
        let pool = SqlitePool::connect(&url)
            .await
            .map_err(|e| AppError::internal("FX_CONN", e.to_string()))?;
        Self::run_migrations(&pool, migration_dir).await?;
        Ok(Self {
            _file: file,
            pool,
            path,
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn run_migrations(pool: &SqlitePool, dir: &str) -> AppResult<()> {
        let p = Path::new(dir);
        let mut entries: Vec<_> = std::fs::read_dir(p)
            .map_err(|e| AppError::internal("FX_DIR", format!("{}: {}", dir, e)))?
            .filter_map(|r| r.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if e.path().extension().and_then(|s| s.to_str()) == Some("sql") {
                let sql = std::fs::read_to_string(e.path())
                    .map_err(|err| AppError::internal("FX_READ", err.to_string()))?;
                sqlx::raw_sql(&sql)
                    .execute(pool)
                    .await
                    .map_err(|err| AppError::internal("FX_EXEC", err.to_string()))?;
            }
        }
        Ok(())
    }
}
