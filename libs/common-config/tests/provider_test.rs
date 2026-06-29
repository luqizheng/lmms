use common_config::provider::ConfigServiceProvider;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetch_returns_map_on_200() {
    let server = MockServer::start().await;
    let body = serde_json::json!({"host": "0.0.0.0", "port": "4000"});

    Mock::given(method("GET"))
        .and(path("/config/export/test-ns"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&server)
        .await;

    let provider = ConfigServiceProvider::new(server.uri(), "test-ns");
    let map = provider.fetch().await.unwrap();

    assert_eq!(map.get("host").unwrap(), "0.0.0.0");
    assert_eq!(map.get("port").unwrap(), "4000");
}

#[tokio::test]
async fn fetch_returns_err_on_404() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/config/export/missing-ns"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let provider = ConfigServiceProvider::new(server.uri(), "missing-ns");
    assert!(provider.fetch().await.is_err());
}

#[tokio::test]
async fn fetch_returns_err_on_invalid_json() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/config/export/bad-json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let provider = ConfigServiceProvider::new(server.uri(), "bad-json");
    assert!(provider.fetch().await.is_err());
}
