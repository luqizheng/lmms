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
#[serde(rename_all = "PascalCase")]
struct AgentServiceRegistration {
    id: String,
    name: String,
    address: String,
    port: u16,
    check: AgentServiceCheck,
    connect: ConnectConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct AgentServiceCheck {
    #[serde(rename = "HTTP")]
    http: String,
    interval: String,
    timeout: String,
    deregister_critical_service_after: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ConnectConfig {
    sidecar_service: SidecarConfig,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "PascalCase")]
struct SidecarConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy: Option<ProxyConfig>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ProxyConfig {
    upstreams: Vec<Upstream>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Upstream {
    destination_name: String,
    local_bind_port: u16,
}

impl Upstream {
    pub fn new(destination_name: &str, local_bind_port: u16) -> Self {
        Self {
            destination_name: destination_name.to_string(),
            local_bind_port,
        }
    }
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
    upstreams: Vec<Upstream>,
}

impl ConsulRegistrar {
    /// Parse `http://host:port` / `https://host[:port]` into `(host, port)`.
    /// Returns `None` on scheme mismatch, missing host, or unparsable port.
    pub fn parse_consul_endpoint(url: &str) -> Option<(String, u16)> {
        let rest = url
            .strip_prefix("http://")
            .map(|s| (s, 850u16))
            .or_else(|| url.strip_prefix("https://").map(|s| (s, 443u16)))?;
        let (host, port) = match rest.0.split_once(':') {
            Some((h, p)) => (h, p),
            None => (rest.0, ""),
        };
        if host.is_empty() {
            return None;
        }
        let parsed_port: u16 = if port.is_empty() {
            rest.1
        } else {
            port.parse().ok()?
        };
        Some((host.to_string(), parsed_port))
    }

    /// Probe the local outbound IP by UDP-binding to `0.0.0.0:0` and connecting
    /// (without sending) to `target`. UDP `connect` only sets the peer address,
    /// so no packet leaves the host. Returns `None` on any failure.
    pub fn probe_local_ip(target: std::net::SocketAddr) -> Option<std::net::IpAddr> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect(target).ok()?;
        let ip = socket.local_addr().ok()?.ip();
        if ip.is_unspecified() {
            return None;
        }
        if let std::net::IpAddr::V6(v6) = ip {
            if v6.is_unicast_link_local() {
                return None;
            }
        }
        Some(ip)
    }

    /// Resolve the best advertise host for Consul registration.
    /// Order: parse `consul_url` → UDP-probe local IP → fallback.
    /// Logs a warning and falls back on any failure.
    pub fn discover_advertise_host(consul_url: &str, fallback: &str) -> String {
        let Some((host, port)) = Self::parse_consul_endpoint(consul_url) else {
            tracing::warn!(
                "failed to parse consul url '{}', using fallback '{}'",
                consul_url,
                fallback
            );
            return fallback.to_string();
        };
        let Ok(target) = format!("{}:{}", host, port).parse::<std::net::SocketAddr>() else {
            tracing::warn!(
                "invalid consul socket '{}:{}', using fallback '{}'",
                host,
                port,
                fallback
            );
            return fallback.to_string();
        };
        match Self::probe_local_ip(target) {
            Some(ip) => {
                tracing::info!("discovered advertise host: {}", ip);
                ip.to_string()
            }
            None => {
                tracing::warn!(
                    "failed to probe local ip, using fallback '{}'",
                    fallback
                );
                fallback.to_string()
            }
        }
    }

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
            upstreams: Vec::new(),
        }
    }

    pub fn with_upstreams(mut self, upstreams: Vec<Upstream>) -> Self {
        self.upstreams = upstreams;
        self
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
        format!("{}:{}", host, port)
    }

    /// Like `register`, but resolves the advertise host via `discover_advertise_host`
    /// using the registrar's `addr` (consul url) and the provided `fallback`.
    /// `fallback` is typically the service name (e.g. `"token-service"`) so consul
    /// can still resolve it via Docker DNS when probe fails.
    pub async fn register_discovered(
        &mut self,
        port: u16,
        fallback: &str,
    ) -> Result<(), String> {
        let host = Self::discover_advertise_host(&self.addr, fallback);
        self.register(&host, port).await
    }

    /// Register against `host:port`. The first call also persists the
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
            connect: ConnectConfig {
                sidecar_service: SidecarConfig {
                    proxy: if self.upstreams.is_empty() {
                        None
                    } else {
                        Some(ProxyConfig {
                            upstreams: self.upstreams.clone(),
                        })
                    },
                },
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
    fn host_to_address_keeps_unspecified() {
        assert_eq!(
            ConsulRegistrar::host_to_address("0.0.0.0", 8082),
            "0.0.0.0:8082"
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

    #[test]
    fn parse_consul_endpoint_strips_http_scheme() {
        let r = ConsulRegistrar::parse_consul_endpoint("http://consul:8500");
        assert_eq!(r, Some(("consul".to_string(), 8500)));
    }

    #[test]
    fn parse_consul_endpoint_strips_https_scheme() {
        let r = ConsulRegistrar::parse_consul_endpoint("https://consul");
        assert_eq!(r, Some(("consul".to_string(), 443)));
    }

    #[test]
    fn parse_consul_endpoint_returns_none_on_garbage() {
        let r = ConsulRegistrar::parse_consul_endpoint("not a url");
        assert_eq!(r, None);
    }

    #[test]
    fn parse_consul_endpoint_returns_none_on_missing_host() {
        let r = ConsulRegistrar::parse_consul_endpoint("http://");
        assert_eq!(r, None);
    }

    #[test]
    fn probe_local_ip_returns_local_ip_when_target_reachable() {
        let probe_target: std::net::SocketAddr = {
            let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let addr = s.local_addr().unwrap();
            std::mem::forget(s);
            addr
        };
        let ip = ConsulRegistrar::probe_local_ip(probe_target);
        assert!(ip.is_some(), "expected Some, got {:?}", ip);
    }

    #[test]
    fn probe_local_ip_returns_none_or_some_without_panic() {
        // port 1 is reserved and likely refuses connection; either Some or None is acceptable.
        let ip = ConsulRegistrar::probe_local_ip("127.0.0.1:1".parse().unwrap());
        let _ = ip;
    }

    #[test]
    fn discover_advertise_host_returns_fallback_on_invalid_url() {
        let r = ConsulRegistrar::discover_advertise_host("not a url", "fallback-host");
        assert_eq!(r, "fallback-host");
    }

    #[test]
    fn discover_advertise_host_returns_loopback_ip_for_local_target() {
        let target_port: u16 = {
            let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let p = s.local_addr().unwrap().port();
            std::mem::forget(s);
            p
        };
        let url = format!("http://127.0.0.1:{}", target_port);
        let ip = ConsulRegistrar::discover_advertise_host(&url, "fallback");
        assert_eq!(ip, "127.0.0.1");
    }

    #[test]
    fn discover_advertise_host_returns_fallback_on_missing_host() {
        let r = ConsulRegistrar::discover_advertise_host("http://", "fb");
        assert_eq!(r, "fb");
    }

    #[tokio::test]
    async fn register_discovered_persists_id_when_consul_unreachable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        let path_str = path.to_str().unwrap();

        let mut r = ConsulRegistrar::new("http://127.0.0.1:1", "x", path_str);
        assert!(!path.exists());

        let _ = r.register_discovered(9999, "x").await;
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, r.service_id());
    }

    #[test]
    fn registration_serializes_plain_service() {
        let reg = AgentServiceRegistration {
            id: "audit-service-abc-uuid".into(),
            name: "audit-service".into(),
            address: "127.0.0.1".into(),
            port: 8080,
            check: AgentServiceCheck {
                http: "http://127.0.0.1:8080/health".into(),
                interval: "10s".into(),
                timeout: "5s".into(),
                deregister_critical_service_after: "1m".into(),
            },
            connect: ConnectConfig {
                sidecar_service: SidecarConfig { proxy: None },
            },
        };
        let json = serde_json::to_value(&reg).unwrap();
        assert_eq!(json["Check"]["HTTP"], "http://127.0.0.1:8080/health");
        assert_eq!(json["Check"]["DeregisterCriticalServiceAfter"], "1m");
        assert_eq!(json["Id"], "audit-service-abc-uuid");
        assert_eq!(json["Name"], "audit-service");
        assert_eq!(json["Connect"]["SidecarService"], serde_json::json!({}));
    }

    #[test]
    fn registration_serializes_connect_sidecar_without_upstreams() {
        let reg = AgentServiceRegistration {
            id: "iam-service-uuid".into(),
            name: "iam-service".into(),
            address: "127.0.0.1".into(),
            port: 18080,
            check: AgentServiceCheck {
                http: "http://127.0.0.1:18080/health".into(),
                interval: "10s".into(),
                timeout: "5s".into(),
                deregister_critical_service_after: "1m".into(),
            },
            connect: ConnectConfig {
                sidecar_service: SidecarConfig { proxy: None },
            },
        };
        let json = serde_json::to_value(&reg).unwrap();
        assert_eq!(json["Connect"]["SidecarService"], serde_json::json!({}));
    }

    #[test]
    fn registration_serializes_connect_sidecar_with_upstreams() {
        let reg = AgentServiceRegistration {
            id: "api-gateway-uuid".into(),
            name: "api-gateway".into(),
            address: "127.0.0.1".into(),
            port: 18080,
            check: AgentServiceCheck {
                http: "http://127.0.0.1:18080/health".into(),
                interval: "10s".into(),
                timeout: "5s".into(),
                deregister_critical_service_after: "1m".into(),
            },
            connect: ConnectConfig {
                sidecar_service: SidecarConfig {
                    proxy: Some(ProxyConfig {
                        upstreams: vec![Upstream {
                            destination_name: "iam-service".into(),
                            local_bind_port: 9081,
                        }],
                    }),
                },
            },
        };
        let json = serde_json::to_value(&reg).unwrap();
        assert_eq!(json["Connect"]["SidecarService"]["Proxy"]["Upstreams"][0]["DestinationName"], "iam-service");
        assert_eq!(json["Connect"]["SidecarService"]["Proxy"]["Upstreams"][0]["LocalBindPort"], 9081);
    }

    #[test]
    fn with_upstreams_stores_upstreams_for_registration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        let r = ConsulRegistrar::new("http://consul:8500", "api-gateway", path.to_str().unwrap())
            .with_upstreams(vec![Upstream {
                destination_name: "iam-service".into(),
                local_bind_port: 9081,
            }]);
        assert_eq!(r.upstreams.len(), 1);
        assert_eq!(r.upstreams[0].destination_name, "iam-service");
    }
}
