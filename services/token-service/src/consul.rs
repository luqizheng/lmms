use crate::config::ConsulConfig;
use bytes::Bytes;
use http_body_util::Full;
use hyper::Request;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use serde::Serialize;
use std::fs;
use uuid::Uuid;

type HttpClient = Client<HttpConnector, Full<Bytes>>;

#[derive(Serialize)]
struct AgentServiceRegistration {
    id: String,
    name: String,
    address: String,
    port: u16,
    check: AgentServiceCheck,
}

#[derive(Serialize)]
struct AgentServiceCheck {
    http: String,
    interval: String,
    timeout: String,
    deregister_critical_service_after: String,
}

pub struct ConsulRegistrar {
    http_client: HttpClient,
    config: ConsulConfig,
    service_id: String,
}

impl ConsulRegistrar {
    pub fn new(config: &ConsulConfig) -> Self {
        let http_client: HttpClient = Client::builder(TokioExecutor::new()).build(HttpConnector::new());
        let service_id = Self::load_or_generate_service_id(&config.service_id_file);

        Self {
            http_client,
            config: config.clone(),
            service_id,
        }
    }

    fn load_or_generate_service_id(file_path: &str) -> String {
        if let Ok(content) = fs::read_to_string(file_path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                tracing::info!("loaded existing service_id from {}", file_path);
                return trimmed.to_string();
            }
        }

        let new_id = format!("{}-{}", "token-service", Uuid::new_v4());
        if let Err(e) = fs::write(file_path, &new_id) {
            tracing::warn!("failed to write service_id to {}: {}", file_path, e);
        } else {
            tracing::info!("generated new service_id: {}", new_id);
        }

        new_id
    }

    fn build_request(&self, url: String, body: Option<Bytes>) -> Result<Request<Full<Bytes>>, String> {
        let mut builder = Request::builder().method("PUT").uri(url);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let req = builder
            .body(Full::new(body.unwrap_or_default()))
            .map_err(|e| format!("build request failed: {}", e))?;
        Ok(req)
    }

    fn host_to_address(host: &str, port: u16) -> String {
        if host == "0.0.0.0" {
            format!("127.0.0.1:{}", port)
        } else {
            format!("{}:{}", host, port)
        }
    }

    pub async fn register(&self, host: &str, port: u16) -> Result<(), String> {
        let address = Self::host_to_address(host, port)
            .split(':')
            .next()
            .unwrap_or("127.0.0.1")
            .to_string();

        let registration = AgentServiceRegistration {
            id: self.service_id.clone(),
            name: self.config.service_name.clone(),
            address,
            port,
            check: AgentServiceCheck {
                http: format!("http://localhost:{}/health", port),
                interval: "10s".to_string(),
                timeout: "5s".to_string(),
                deregister_critical_service_after: "1m".to_string(),
            },
        };

        let url = format!("{}/v1/agent/service/register", self.config.addr);
        let body = Bytes::from(serde_json::to_vec(&registration).map_err(|e| e.to_string())?);
        let req = self.build_request(url, Some(body))?;

        match self.http_client.request(req).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    tracing::info!(
                        "successfully registered service {} with id {} at {}:{}",
                        self.config.service_name,
                        self.service_id,
                        host,
                        port
                    );
                    Ok(())
                } else {
                    let err_msg = format!("failed to register service: HTTP {}", resp.status());
                    tracing::error!("{}", err_msg);
                    Err(err_msg)
                }
            }
            Err(e) => {
                let err_msg = format!("failed to register service: {}", e);
                tracing::error!("{}", err_msg);
                Err(err_msg)
            }
        }
    }

    pub async fn deregister(&self) -> Result<(), String> {
        let url = format!(
            "{}/v1/agent/service/deregister/{}",
            self.config.addr, self.service_id
        );
        let req = self.build_request(url, None)?;

        match self.http_client.request(req).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    tracing::info!("successfully deregistered service {}", self.service_id);
                    Ok(())
                } else {
                    let err_msg = format!("failed to deregister service: HTTP {}", resp.status());
                    tracing::warn!("{}", err_msg);
                    Err(err_msg)
                }
            }
            Err(e) => {
                let err_msg = format!("failed to deregister service: {}", e);
                tracing::warn!("{}", err_msg);
                Err(err_msg)
            }
        }
    }

    pub fn service_id(&self) -> &str {
        &self.service_id
    }
}
