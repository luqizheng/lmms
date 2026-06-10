use axum::http::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceInstance {
    pub address: String,
    pub port: u16,
    pub service_name: String,
}

pub struct ServiceDiscovery {
    consul_addr: String,
    cache: Arc<RwLock<HashMap<String, ServiceInstance>>>,
}

impl ServiceDiscovery {
    pub fn new(consul_addr: &str) -> Self {
        Self {
            consul_addr: consul_addr.to_string(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn discover(&self, service_name: &str) -> Result<ServiceInstance, StatusCode> {
        // 先查缓存
        {
            let cache = self.cache.read().await;
            if let Some(instance) = cache.get(service_name) {
                return Ok(instance.clone());
            }
        }

        // 从 Consul 查询
        let url = format!(
            "{}/v1/health/service/{}?passing=true",
            self.consul_addr, service_name
        );

        let resp = reqwest::get(&url)
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

        #[derive(Deserialize)]
        struct ConsulService {
            service: ConsulServiceInfo,
        }
        #[derive(Deserialize)]
        struct ConsulServiceInfo {
            address: String,
            port: u16,
        }

        let services: Vec<ConsulService> = resp
            .json()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

        let instance = services.first().ok_or(StatusCode::SERVICE_UNAVAILABLE).map(|s| ServiceInstance {
            address: s.service.address.clone(),
            port: s.service.port,
            service_name: service_name.to_string(),
        })?;

        // 写入缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(service_name.to_string(), instance.clone());
        }

        Ok(instance)
    }
}
