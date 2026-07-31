use digua_http_client::client::KudiWebClient;
use digua_http_client::models::TokenInfo;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn end_to_end_service_to_service_via_wiremock() {
    let server = MockServer::start().await;
    let token_url = format!("{}/api/tokens/access", server.uri());
    let downstream_url = format!("{}/api/users/123", server.uri());

    Mock::given(method("POST"))
        .and(path("/api/tokens/access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(TokenInfo {
            access_token: "real-token".into(),
            refresh_token: None,
            token_type: "Bearer".into(),
            expires_in: 3600,
        }))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/users/123"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KudiWebClient::builder()
        .credential("svc-a", "secret-a")
        .token_endpoint(token_url)
        .read_timeout_ms(5000)
        .build()
        .unwrap();

    let resp = client.get(&downstream_url, None).await.unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, "ok");
}
