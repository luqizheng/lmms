use digua_common_http::HttpClient;
use std::time::Duration;

#[test]
fn client_builder_has_request_id() {
    let c = HttpClient::builder().default_request_id("abc-1").build();
    assert!(c.is_ok());
    assert_eq!(c.unwrap().request_id_header(), "X-Request-Id");
}

#[test]
fn client_builder_without_request_id() {
    let c = HttpClient::builder().build();
    assert!(c.is_ok());
    assert_eq!(c.unwrap().request_id_header(), "X-Request-Id");
}

#[test]
fn client_builder_is_chainable() {
    let c = HttpClient::builder()
        .timeout(Duration::from_secs(15))
        .default_request_id("test-request-id")
        .build();
    assert!(c.is_ok());
    assert_eq!(c.unwrap().request_id_header(), "X-Request-Id");
}
