use common_http::HttpClient;

#[tokio::test]
async fn client_builds_with_request_id_header() {
    let c = HttpClient::builder().request_id("abc-1").build().unwrap();
    assert_eq!(c.default_request_id(), Some("abc-1"));
}