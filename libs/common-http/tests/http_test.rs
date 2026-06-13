use common_http::HttpClient;
use std::time::Duration;

#[test]
fn client_builder_has_request_id() {
    let c = HttpClient::builder().request_id("abc-1").build();
    assert!(c.is_ok());
    assert_eq!(c.unwrap().default_request_id(), Some("abc-1"));
}

#[test]
fn client_builder_without_request_id() {
    let c = HttpClient::builder().build();
    assert!(c.is_ok());
    assert_eq!(c.unwrap().default_request_id(), None);
}

#[test]
fn client_builder_is_chainable() {
    let c = HttpClient::builder()
        .timeout(Duration::from_secs(15))
        .request_id("test-request-id")
        .build();
    assert!(c.is_ok());
    assert_eq!(c.unwrap().default_request_id(), Some("test-request-id"));
}
