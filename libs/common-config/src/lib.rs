use common_error::{AppError, AppResult};
use config::{Config, Environment};
use serde::de::DeserializeOwned;

pub struct Loader {
    prefix: String,
    file: Option<String>,
}

impl Loader {
    pub fn new(env_prefix: impl Into<String>) -> Self {
        Self {
            prefix: env_prefix.into(),
            file: None,
        }
    }
    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.file = Some(path.into());
        self
    }
    pub fn load<T: DeserializeOwned>(self) -> AppResult<T> {
        let mut b = Config::builder();
        if let Some(f) = self.file {
            b = b.add_source(config::File::with_name(&f).required(false));
        }
        b = b.add_source(Environment::with_prefix(&self.prefix).separator("_"));
        let c = b
            .build()
            .map_err(|e| AppError::internal("CFG_BUILD", e.to_string()))?;
        c.try_deserialize::<T>()
            .map_err(|e| AppError::internal("CFG_PARSE", e.to_string()))
    }
}
