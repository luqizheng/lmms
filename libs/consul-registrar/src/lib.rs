//! Consul service registration helper.
//!
//! `ConsulRegistrar` PUTs `/v1/agent/service/register` on `new().register(host, port)`,
//! and DELETEs `/v1/agent/service/deregister/{id}` on `deregister()`. The service id
//! is persisted to `service_id_file` so restarts keep the same id.

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

/// Consul agent service registrar. Created once at startup, registered against
/// the local address, then deregistered on shutdown.
///
/// `service_id` lifecycle:
/// - `new()` only **reads** `service_id_file`. If the file has a non-empty value, that
///   value is reused; otherwise a fresh `{service_name}-{uuid}` id is generated and
///   kept in memory (no fs write yet).
/// - `register()` persists the id to `service_id_file` on its first successful or
///   attempted call. Subsequent restarts load the same id back from disk, so the
///   service id stays stable across container restarts as long as the file path
///   is on persistent storage (NOT `/tmp` on a recreated container).
pub struct ConsulRegistrar {
    http_client: HttpClient,
    addr: String,
    service_name: String,
    service_id: String,
    service_id_file: String,
    persisted: bool,
}

impl ConsulRegistrar {
    /// Build a registrar. `addr` is the consul agent base URL (e.g. `http://consul:8500`).
    /// `service_id_file` is the on-disk path that holds the service id across restarts.
    /// Pass a path on a persistent volume (not `/tmp` on a recreated container) if you
    /// need the id to survive `docker compose down && up`.
    pub fn new(addr: &str, service_name: &str, service_id_file: &str) -> Self {
        let http_client: HttpClient =
            Client::builder(TokioExecutor::new()).build(HttpConnector::new());
        let (service_id, persisted) = Self::load_service_id(service_name, service_id_file);

        Self {
            http_client,
            addr: addr.to_string(),
            service_name: service_name.to_string(),
            service_id,
            service_id_file: service_id_file.to_string(),
            persisted,
        }
    }

    /// Returns `(service_id, persisted)`. `persisted = true` means the value was read
    /// from disk and is already durable; `false` means a fresh id was generated and
    /// still needs to be written.
    fn load_service_id(service_name: &str, file_path: &str) -> (String, bool) {
        if let Ok(content) = fs::read_to_string(file_path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                tracing::info!("loaded existing service_id from {}", file_path);
                return (trimmed.to_string(), true);
            }
        }

        let new_id = format!("{}-{}", service_name, Uuid::new_v4());
        tracing::info!("generated new in-memory service_id: {}", new_id);
        (new_id, false)
    }

    /// Write the current `service_id` to disk if not already persisted. Idempotent.
    fn persist(&mut self) {
        if self.persisted {
            return;
        }
        match fs::write(&self.service_id_file, &self.service_id) {
            Ok(_) => {
                tracing::info!(
                    "persisted service_id {} to {}",
                    self.service_id,
                    self.service_id_file
                );
                self.persisted = true;
            }
            Err(e) => {
                tracing::warn!(
                    "failed to persist service_id to {}: {}. service_id will rotate on next restart",
                    self.service_id_file,
                    e
                );
            }
        }
    }

    fn build_request(
        &self,
        url: String,
        body: Option<Bytes>,
    ) -> Result<Request<Full<Bytes>>, String> {
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

    /// Register against `host:port`. `host = "0.0.0.0"` is rewritten to `127.0.0.1`
    /// for the consul-facing `address` field. The first call also persists the
    /// service id to `service_id_file` so a container restart reuses the same id.
    pub async fn register(&mut self, host: &str, port: u16) -> Result<(), String> {
        // Persist eagerly: even if consul is unreachable, the id is locked in for the
        // next restart's retry instead of rotating on every boot.
        self.persist();

        let address = Self::host_to_address(host, port)
            .split(':')
            .next()
            .unwrap_or("127.0.0.1")
            .to_string();

        let registration = AgentServiceRegistration {
            id: self.service_id.clone(),
            name: self.service_name.clone(),
            address: address.clone(),
            port,
            check: AgentServiceCheck {
                http: format!("http://{}:{}/health", address, port),
                interval: "10s".to_string(),
                timeout: "5s".to_string(),
                deregister_critical_service_after: "1m".to_string(),
            },
        };

        let url = format!("{}/v1/agent/service/register", self.addr);
        let body = Bytes::from(serde_json::to_vec(&registration).map_err(|e| e.to_string())?);
        let req = self.build_request(url, Some(body))?;

        match self.http_client.request(req).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    tracing::info!(
                        "successfully registered service {} with id {} at {}:{}",
                        self.service_name,
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

    /// Deregister by id. Best-effort: errors are surfaced for logging.
    pub async fn deregister(&self) -> Result<(), String> {
        let url = format!(
            "{}/v1/agent/service/deregister/{}",
            self.addr, self.service_id
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

    /// Returns the persisted/generated service id.
    pub fn service_id(&self) -> &str {
        &self.service_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_to_address_rewrites_unspecified() {
        assert_eq!(
            ConsulRegistrar::host_to_address("0.0.0.0", 8082),
            "127.0.0.1:8082"
        );
    }

    #[test]
    fn host_to_address_keeps_named_host() {
        assert_eq!(
            ConsulRegistrar::host_to_address("user-service", 9000),
            "user-service:9000"
        );
    }

    #[test]
    fn service_id_is_persisted_across_calls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        let path_str = path.to_str().unwrap();

        let first = ConsulRegistrar::new("http://consul:8500", "user-service", path_str);
        let second = ConsulRegistrar::new("http://consul:8500", "user-service", path_str);

        assert_eq!(first.service_id(), second.service_id());
        assert!(first.service_id().starts_with("user-service-"));
    }

    #[test]
    fn service_id_overwritten_when_file_blank() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        std::fs::write(&path, "").unwrap();
        let path_str = path.to_str().unwrap();

        let r = ConsulRegistrar::new("http://consul:8500", "x", path_str);
        assert!(r.service_id().starts_with("x-"));
    }

    #[test]
    fn new_does_not_write_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        let path_str = path.to_str().unwrap();

        let _ = ConsulRegistrar::new("http://consul:8500", "x", path_str);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn register_persists_id_even_when_consul_unreachable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        let path_str = path.to_str().unwrap();

        let mut r = ConsulRegistrar::new("http://127.0.0.1:1", "x", path_str);
        assert!(!path.exists());

        // consul is not listening on port 1; the http call will fail, but the id
        // must already be on disk by the time register() returns.
        let _ = r.register("127.0.0.1", 9999).await;
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, r.service_id());
    }

    #[tokio::test]
    async fn register_reuses_id_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        let path_str = path.to_str().unwrap();

        let mut first = ConsulRegistrar::new("http://127.0.0.1:1", "x", path_str);
        let _ = first.register("127.0.0.1", 9999).await;
        let id_after_first_register = first.service_id().to_string();

        // Simulate process restart: brand-new instance reading the same file.
        let second = ConsulRegistrar::new("http://127.0.0.1:1", "x", path_str);
        assert_eq!(second.service_id(), id_after_first_register);
    }
}
