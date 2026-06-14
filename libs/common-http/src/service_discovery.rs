use common_error::{AppError, AppResult};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceInstance {
    pub id: String,
    pub service: String,
    pub address: String,
    pub port: u16,
    pub tags: Option<Vec<String>>,
    pub meta: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ConsulHealthResponse {
    #[serde(rename = "ServiceInstance")]
    service_instance: ServiceInstance,
}

#[derive(Debug, Clone)]
pub struct ConsulResolver {
    client: Client,
    consul_url: String,
    cache: Arc<RwLock<HashMap<String, Vec<ServiceInstance>>>>,
    cache_ttl_secs: u64,
}

impl ConsulResolver {
    pub fn new(consul_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            consul_url: consul_url.into().trim_end_matches('/').to_string(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl_secs: 30,
        }
    }

    pub fn cache_ttl(mut self, secs: u64) -> Self {
        self.cache_ttl_secs = secs;
        self
    }

    pub async fn resolve(&self, service_name: &str) -> AppResult<Vec<ServiceInstance>> {
        let cache_key = service_name.to_string();
        
        {
            let cache = self.cache.read().await;
            if let Some(instances) = cache.get(&cache_key) {
                return Ok(instances.clone());
            }
        }

        let url = format!(
            "{}/v1/health/service/{}?passing=true",
            self.consul_url, service_name
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::external("CONSUL_QUERY", e.to_string()))?;

        if !response.status().is_success() {
            return Err(AppError::external(
                "CONSUL_HTTP",
                format!("Consul returned {}", response.status()),
            ));
        }

        let consul_response: Vec<ConsulHealthResponse> = response
            .json()
            .await
            .map_err(|e| AppError::external("CONSUL_PARSE", e.to_string()))?;

        let instances: Vec<ServiceInstance> = consul_response
            .into_iter()
            .map(|r| r.service_instance)
            .collect();

        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, instances.clone());
        }

        Ok(instances)
    }

    pub async fn resolve_one(&self, service_name: &str) -> AppResult<ServiceInstance> {
        let instances = self.resolve(service_name).await?;
        
        instances
            .into_iter()
            .next()
            .ok_or_else(|| AppError::not_found("SERVICE_NOT_FOUND", service_name))
    }

    pub async fn resolve_url(&self, service_name: &str) -> AppResult<String> {
        let instance = self.resolve_one(service_name).await?;
        Ok(format!("http://{}:{}", instance.address, instance.port))
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consul_resolver_creation() {
        let resolver = ConsulResolver::new("http://localhost:8500");
        assert_eq!(resolver.consul_url, "http://localhost:8500");
    }

    #[test]
    fn test_consul_resolver_trims_slash() {
        let resolver = ConsulResolver::new("http://localhost:8500/");
        assert_eq!(resolver.consul_url, "http://localhost:8500");
    }

    #[tokio::test]
    async fn test_cache_clears() {
        let resolver = ConsulResolver::new("http://localhost:8500");
        resolver.clear_cache().await;
    }
}
