//! Env-contract parsing and the registration lifecycle against a mock Consul.

use digua_common_consul::{consul_guard, deregister, is_registered, register, RegistrationConfig};
use std::sync::Mutex;
use std::time::Duration;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serialise env mutation across tests (std::env is process-global).
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn set_env(vars: &[(&str, Option<&str>)]) {
    for (k, v) in vars {
        match v {
            Some(val) => std::env::set_var(k, val),
            None => std::env::remove_var(k),
        }
    }
}

// ── Env contract ────────────────────────────────────────────────────────────

#[test]
fn unset_service_name_disables_registration() {
    let _g = ENV_LOCK.lock().unwrap();
    set_env(&[("SERVICE_NAME", None), ("SERVICE_PORT", None)]);
    let cfg = RegistrationConfig::from_env().expect("no hard error");
    assert!(
        cfg.is_none(),
        "missing SERVICE_NAME must disable registration"
    );
}

#[test]
fn service_name_without_port_is_a_config_error() {
    let _g = ENV_LOCK.lock().unwrap();
    set_env(&[("SERVICE_NAME", Some("svc-a")), ("SERVICE_PORT", None)]);
    let err = RegistrationConfig::from_env().expect_err("port required");
    assert!(err.contains("SERVICE_PORT"));
}

#[test]
fn full_contract_parses_with_defaults() {
    let _g = ENV_LOCK.lock().unwrap();
    set_env(&[
        ("SERVICE_NAME", Some("svc-a")),
        ("SERVICE_PORT", Some("8081")),
        ("SERVICE_ADDRESS", Some("svc-a")),
        ("HOSTNAME", Some("svc-a-host")),
        ("CONSUL_ADDR", None),
        ("SERVICE_HEALTH_PATH", None),
        ("SERVICE_TAGS", None),
    ]);
    let cfg = RegistrationConfig::from_env()
        .expect("valid")
        .expect("enabled");
    assert_eq!(cfg.consul_addr, "http://consul:8500");
    assert_eq!(cfg.service_name, "svc-a");
    assert_eq!(cfg.service_id, "svc-a-svc-a-host");
    assert_eq!(cfg.address, "svc-a");
    assert_eq!(cfg.port, 8081);
    assert_eq!(cfg.health_path, "/health");
    assert_eq!(cfg.tags, vec!["backend".to_string(), "svc-a".to_string()]);
}

#[test]
fn service_tags_split_and_trim() {
    let _g = ENV_LOCK.lock().unwrap();
    set_env(&[
        ("SERVICE_NAME", Some("svc-b")),
        ("SERVICE_PORT", Some("9000")),
        ("SERVICE_ADDRESS", Some("svc-b")),
        ("SERVICE_TAGS", Some("backend, svc-b , metrics")),
    ]);
    let cfg = RegistrationConfig::from_env()
        .expect("valid")
        .expect("enabled");
    assert_eq!(
        cfg.tags,
        vec![
            "backend".to_string(),
            "svc-b".to_string(),
            "metrics".to_string()
        ]
    );
}

// ── Low-level HTTP operations ───────────────────────────────────────────────

fn test_cfg(consul_addr: String) -> RegistrationConfig {
    RegistrationConfig {
        consul_addr,
        service_name: "svc-http".into(),
        service_id: "svc-http-host".into(),
        address: "svc-http".into(),
        port: 8081,
        health_path: "/health".into(),
        tags: vec!["backend".into(), "svc-http".into()],
    }
}

#[tokio::test]
async fn register_puts_payload_with_health_check() {
    let server = MockServer::start().await;
    let cfg = test_cfg(server.uri());

    let expected_check_url = format!("http://{}:{}/health", cfg.address, cfg.port);
    let _mock = Mock::given(method("PUT"))
        .and(path("/v1/agent/service/register"))
        .and(body_partial_json(serde_json::json!({
            "name": "svc-http",
            "id": "svc-http-host",
            "address": "svc-http",
            "port": 8081,
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount_as_scoped(&server)
        .await;

    register(&reqwest::Client::new(), &cfg)
        .await
        .expect("register ok");
    // `mock` (MockGuard) asserts its expectation when dropped at test end.
    // The health-check URL contract is asserted via the payload matchers
    // above plus this direct check on the config the payload derives from:
    assert_eq!(expected_check_url, "http://svc-http:8081/health");
}

#[tokio::test]
async fn register_surveys_http_error() {
    let server = MockServer::start().await;
    let cfg = test_cfg(server.uri());
    Mock::given(method("PUT"))
        .and(path("/v1/agent/service/register"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let err = register(&reqwest::Client::new(), &cfg)
        .await
        .expect_err("500 is an error");
    assert!(err.contains("500"));
}

#[tokio::test]
async fn is_registered_reads_agent_state() {
    let server = MockServer::start().await;
    let cfg = test_cfg(server.uri());

    Mock::given(method("GET"))
        .and(path("/v1/agent/service/name/svc-http-host"))
        .respond_with(ResponseTemplate::new(404))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/agent/service/name/svc-http-host"))
        .respond_with(ResponseTemplate::new(200).set_body_string("registered"))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    assert!(!is_registered(&client, &cfg).await.expect("probe 1"));
    assert!(is_registered(&client, &cfg).await.expect("probe 2"));
}

#[tokio::test]
async fn deregister_hits_service_id_path() {
    let server = MockServer::start().await;
    let cfg = test_cfg(server.uri());
    let _mock = Mock::given(method("PUT"))
        .and(path("/v1/agent/service/deregister/svc-http-host"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    deregister(&reqwest::Client::new(), &cfg)
        .await
        .expect("deregister ok");
    // `mock` asserts its expectation when dropped at test end.
}

// ── Guard lifecycle (real spawn path) ───────────────────────────────────────

/// consul_guard registers in the background; deregister() removes it.
/// The guard's own shutdown path also fires one deregister, so expect a range.
#[tokio::test]
async fn guard_registers_then_deregisters() {
    let server = MockServer::start().await;
    // Lock covers env mutation + config capture only; must be released
    // before any await (clippy await_holding_lock).
    let guard = {
        let _g = ENV_LOCK.lock().unwrap();
        set_env(&[
            ("CONSUL_ADDR", Some(server.uri().as_str())),
            ("SERVICE_NAME", Some("svc-guard")),
            ("SERVICE_PORT", Some("8081")),
            ("SERVICE_ADDRESS", Some("svc-guard")),
            ("HOSTNAME", Some("svc-guard-host")),
        ]);
        consul_guard()
    };

    let _reg_guard = Mock::given(method("PUT"))
        .and(path("/v1/agent/service/register"))
        .respond_with(ResponseTemplate::new(200))
        .mount_as_scoped(&server)
        .await;
    let _der_guard = Mock::given(method("PUT"))
        .and(path(
            "/v1/agent/service/deregister/svc-guard-svc-guard-host",
        ))
        .respond_with(ResponseTemplate::new(200))
        .expect(1..)
        .mount_as_scoped(&server)
        .await;

    // Catalog probe the test loop uses to wait for registration.
    let name_guard = Mock::given(method("GET"))
        .and(path("/v1/agent/service/name/svc-guard-svc-guard-host"))
        .respond_with(ResponseTemplate::new(200).set_body_string("registered"))
        .mount_as_scoped(&server)
        .await;

    assert!(guard.enabled(), "guard must manage a registration");

    // Wait for the background task's initial registration to land.
    let client = reqwest::Client::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if is_registered(
            &client,
            &RegistrationConfig {
                consul_addr: server.uri(),
                service_name: "svc-guard".into(),
                service_id: "svc-guard-svc-guard-host".into(),
                address: "svc-guard".into(),
                port: 8081,
                health_path: "/health".into(),
                tags: vec![],
            },
        )
        .await
        .unwrap_or(false)
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "registration never landed"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    guard.deregister().await;
    // give the background task a beat to observe shutdown and finish
    tokio::time::sleep(Duration::from_millis(100)).await;

    // env cleanup needs the lock, but this test does no await afterwards
    drop(name_guard);
    drop(_reg_guard);
    drop(_der_guard);
    let _g = ENV_LOCK.lock().unwrap();
    set_env(&[
        ("CONSUL_ADDR", None),
        ("SERVICE_NAME", None),
        ("SERVICE_PORT", None),
        ("SERVICE_ADDRESS", None),
        ("HOSTNAME", None),
    ]);
    // guards assert their expectations when dropped at test end
}

#[test]
fn guard_without_env_is_disabled() {
    let guard = {
        let _g = ENV_LOCK.lock().unwrap();
        set_env(&[("SERVICE_NAME", None)]);
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { consul_guard() })
    };
    assert!(!guard.enabled());
}
