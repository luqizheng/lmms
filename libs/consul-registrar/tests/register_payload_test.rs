//! 集成测试：用 wiremock 模拟 Consul agent，截获 PUT /v1/agent/service/register
//! 实际发出的字节，验证 Connect.SidecarService.ID 使用 service_name（不带 uuid）。

use serde_json::Value;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

#[tokio::test]
async fn register_payload_includes_connect_sidecar_with_fixed_id() {
    let server = MockServer::start().await;
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<Value>));
    let cap_clone = captured.clone();

    Mock::given(method("PUT"))
        .and(path("/v1/agent/service/register"))
        .respond_with(move |req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).expect("body json");
            *cap_clone.lock().unwrap() = Some(body);
            ResponseTemplate::new(200)
        })
        .mount(&server)
        .await;

    let tmp = tempfile::tempdir().unwrap();
    let id_path = tmp.path().join("svc-id").to_string_lossy().to_string();
    let mut r = consul_registrar::ConsulRegistrar::new(&server.uri(), "audit-service", &id_path);
    let _ = r.register("127.0.0.1", 8080).await;

    let body = captured.lock().unwrap().clone().expect("register not called");
    assert_eq!(body["Connect"]["SidecarService"]["Id"], "audit-service-sidecar-proxy");
    assert_eq!(body["Connect"]["SidecarService"]["Name"], "audit-service-sidecar-proxy");
    assert_eq!(body["Name"], "audit-service");
    assert!(
        body["Id"].as_str().unwrap().starts_with("audit-service-"),
        "parent id should keep uuid suffix, got {}",
        body["Id"]
    );
}
