use anyhow::{bail, Context};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
    pub log_level: String,
    pub storage_path: String,
    pub storage_backend: String,
    pub max_file_size: u64,
    pub s3_bucket: String,
    pub s3_region: String,
    pub s3_endpoint: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub s3_path_style: bool,
    pub s3_auto_create_bucket: bool,
}

fn default_server_addr() -> String {
    std::env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:8082".to_string())
}

fn default_log_level() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

fn default_storage_path() -> String {
    std::env::var("STORAGE_PATH").unwrap_or_else(|_| "./data/files".to_string())
}

fn default_storage_backend() -> String {
    std::env::var("FILE_STORAGE_BACKEND").unwrap_or_else(|_| "local".to_string())
}

fn default_max_file_size() -> u64 {
    100 * 1024 * 1024
}

fn default_s3_bucket() -> String {
    std::env::var("S3_BUCKET").unwrap_or_else(|_| "coder-files".to_string())
}

fn default_s3_region() -> String {
    std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string())
}

fn default_s3_path_style() -> bool {
    matches!(
        std::env::var("S3_PATH_STYLE").as_deref(),
        Ok("true") | Ok("1") | Ok("yes")
    )
}

fn default_s3_auto_create_bucket() -> bool {
    !matches!(
        std::env::var("S3_AUTO_CREATE_BUCKET").as_deref(),
        Ok("false") | Ok("0") | Ok("no")
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    Local,
    Rustfs,
    S3,
}

impl StorageBackend {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "local" => Some(Self::Local),
            "rustfs" => Some(Self::Rustfs),
            "s3" | "aws" | "oss" | "minio" => Some(Self::S3),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Rustfs => "rustfs",
            Self::S3 => "s3",
        }
    }

    pub fn path_style_default(&self) -> bool {
        match self {
            Self::Local => false,
            Self::Rustfs => true,
            Self::S3 => false,
        }
    }
}

impl AppConfig {
    /// DATABASE_URL is required — no dev-DB fallback.
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL required — file-service has no fallback")?;
        if database_url.trim().is_empty() {
            bail!("DATABASE_URL is set but empty");
        }

        Ok(Self {
            database_url,
            server_addr: default_server_addr(),
            log_level: default_log_level(),
            storage_path: default_storage_path(),
            storage_backend: default_storage_backend(),
            max_file_size: default_max_file_size(),
            s3_bucket: default_s3_bucket(),
            s3_region: default_s3_region(),
            s3_endpoint: std::env::var("S3_ENDPOINT")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            s3_access_key: std::env::var("S3_ACCESS_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            s3_secret_key: std::env::var("S3_SECRET_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            s3_path_style: default_s3_path_style(),
            s3_auto_create_bucket: default_s3_auto_create_bucket(),
        })
    }

    pub fn backend(&self) -> anyhow::Result<StorageBackend> {
        StorageBackend::parse(&self.storage_backend).context(format!(
            "FILE_STORAGE_BACKEND='{}' is not recognized; expected local|rustfs|s3",
            self.storage_backend
        ))
    }

    pub fn s3_config(&self) -> anyhow::Result<crate::storage::S3Config> {
        let backend = self.backend()?;
        let endpoint = match backend {
            StorageBackend::Local => {
                bail!("s3_config() called but backend=local");
            }
            StorageBackend::Rustfs => self.s3_endpoint.clone().ok_or_else(|| {
                anyhow::anyhow!("rustfs backend requires S3_ENDPOINT env (e.g. http://rustfs:9000)")
            })?,
            StorageBackend::S3 => self
                .s3_endpoint
                .clone()
                .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", self.s3_region)),
        };

        let access_key = self
            .s3_access_key
            .clone()
            .ok_or_else(|| anyhow::anyhow!("S3_ACCESS_KEY is required when backend != local"))?;
        let secret_key = self
            .s3_secret_key
            .clone()
            .ok_or_else(|| anyhow::anyhow!("S3_SECRET_KEY is required when backend != local"))?;

        Ok(crate::storage::S3Config {
            bucket_name: self.s3_bucket.clone(),
            region: self.s3_region.clone(),
            endpoint: Some(endpoint),
            access_key,
            secret_key,
            path_style: backend.path_style_default() || self.s3_path_style,
            auto_create_bucket: self.s3_auto_create_bucket,
        })
    }
}
