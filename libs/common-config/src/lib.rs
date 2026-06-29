#[cfg(feature = "config-service")]
pub mod provider;

use common_error::{AppError, AppResult};
use config::{Config, Environment, File, FileFormat};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "config-service")]
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct Loader {
    prefix: String,
    file: Option<String>,
    consul_url: Option<String>,
    consul_key: Option<String>,
    #[cfg(feature = "config-service")]
    config_service_url: Option<String>,
    #[cfg(feature = "config-service")]
    config_service_namespace: Option<String>,
}

impl Loader {
    pub fn new(env_prefix: impl Into<String>) -> Self {
        Self {
            prefix: env_prefix.into(),
            file: None,
            consul_url: None,
            consul_key: None,
            #[cfg(feature = "config-service")]
            config_service_url: None,
            #[cfg(feature = "config-service")]
            config_service_namespace: None,
        }
    }

    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.file = Some(path.into());
        self
    }

    pub fn with_consul(mut self, url: impl Into<String>, key: impl Into<String>) -> Self {
        self.consul_url = Some(url.into());
        self.consul_key = Some(key.into());
        self
    }

    #[cfg(feature = "config-service")]
    pub fn with_config_service(
        mut self,
        url: impl Into<String>,
        namespace: impl Into<String>,
    ) -> Self {
        self.config_service_url = Some(url.into());
        self.config_service_namespace = Some(namespace.into());
        self
    }

    pub fn load<T: DeserializeOwned>(self) -> AppResult<T> {
        let mut builder = Config::builder();

        if let Some(f) = self.file {
            builder = builder.add_source(File::with_name(&f).required(false));
        }

        #[cfg(feature = "config-service")]
        if let (Some(url), Some(ns)) = (
            self.config_service_url.as_ref(),
            self.config_service_namespace.as_ref(),
        ) {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| AppError::internal("CFG_RUNTIME", e.to_string()))?;
            match rt.block_on(provider::ConfigServiceProvider::new(url, ns).fetch()) {
                Ok(map) => {
                    for (k, v) in &map {
                        builder = builder
                            .set_override(k.as_str(), v.as_str())
                            .map_err(|e| AppError::internal("CFG_OVERRIDE", e.to_string()))?;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "config-service fetch failed, using local config");
                }
            }
        }

        builder = builder.add_source(Environment::with_prefix(&self.prefix).separator("_"));

        #[cfg(feature = "consul")]
        if let (Some(url), Some(key)) = (self.consul_url, self.consul_key) {
            let consul_value = load_from_consul(&url, &key)?;
            builder = builder.add_source(
                config::Value::new(None, serde_json::from_str(&consul_value).map_err(|e| {
                    AppError::internal("CFG_CONSUL_PARSE", e.to_string())
                })?),
            );
        }

        let config = builder
            .build()
            .map_err(|e| AppError::internal("CFG_BUILD", e.to_string()))?;

        config.try_deserialize::<T>().map_err(|e| {
            AppError::internal("CFG_PARSE", e.to_string())
        })
    }
}

#[cfg(feature = "config-service")]
pub fn watch_config<T: DeserializeOwned + Serialize + Clone + Send + Sync + 'static>(
    provider: provider::ConfigServiceProvider,
    hot: Arc<HotConfig<T>>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        loop {
            tick.tick().await;
            match provider.fetch().await {
                Ok(map) => {
                    let new_value = serde_json::to_value(&map).unwrap_or_default();
                    let current = serde_json::to_value(hot.get().await).unwrap_or_default();
                    if new_value != current {
                        if let Ok(deserialized) = serde_json::from_value::<T>(new_value) {
                            hot.update(deserialized).await;
                            tracing::info!("hot config updated via config-service poll");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "config-service poll failed");
                }
            }
        }
    })
}

#[cfg(feature = "consul")]
fn load_from_consul(url: &str, key: &str) -> AppResult<String> {
    let client = reqwest::blocking::Client::new();
    let res = client
        .get(&format!("{}/v1/kv/{}", url, key))
        .send()
        .map_err(|e| AppError::external("CFG_CONSUL_HTTP", e.to_string()))?;

    if !res.status().is_success() {
        return Err(AppError::external(
            "CFG_CONSUL_NOT_FOUND",
            format!("consul key not found: {}", key),
        ));
    }

    let response: Vec<serde_json::Value> = res
        .json()
        .map_err(|e| AppError::external("CFG_CONSUL_PARSE", e.to_string()))?;

    if let Some(first) = response.first() {
        if let Some(value) = first.get("Value") {
            if let Some(s) = value.as_str() {
                return Ok(base64::decode(s)
                    .map_err(|e| AppError::internal("CFG_CONSUL_DECODE", e.to_string()))
                    .map(String::from_utf8_lossy)
                    .map(|s| s.to_string())?);
            }
        }
    }

    Err(AppError::external("CFG_CONSUL_EMPTY", "consul value empty"))
}

pub struct HotConfig<T> {
    inner: Arc<RwLock<T>>,
    callbacks: Arc<RwLock<Vec<Box<dyn Fn(&T) + Send + Sync>>>>,
}

impl<T: DeserializeOwned + Clone + Send + Sync + 'static> HotConfig<T> {
    pub async fn new(initial: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
            callbacks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn get(&self) -> T {
        self.inner.read().await.clone()
    }

    pub async fn update(&self, new_value: T) {
        {
            let mut inner = self.inner.write().await;
            *inner = new_value.clone();
        }

        let callbacks = self.callbacks.read().await;
        for cb in callbacks.iter() {
            cb(&new_value);
        }
    }

    pub async fn on_change<F: Fn(&T) + Send + Sync + 'static>(&self, callback: F) {
        self.callbacks.write().await.push(Box::new(callback));
    }
}

#[derive(Serialize, Deserialize)]
pub struct Secret<T>(T);

impl<T: std::fmt::Display> std::fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "******")
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret(******)")
    }
}

impl<T> Secret<T> {
    pub fn inner(&self) -> &T {
        &self.0
    }
}

pub struct SensitiveFields {
    fields: HashMap<String, bool>,
}

impl SensitiveFields {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    pub fn add(&mut self, field: &str) {
        self.fields.insert(field.to_string(), true);
    }

    pub fn is_sensitive(&self, field: &str) -> bool {
        self.fields.contains_key(field)
    }

    pub fn mask_value(&self, field: &str, value: &str) -> String {
        if self.is_sensitive(field) {
            "******".to_string()
        } else {
            value.to_string()
        }
    }
}