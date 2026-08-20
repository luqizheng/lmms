use async_trait::async_trait;
use digua_http_client::client::KudiWebClient;
use digua_http_client::error::KudiHttpError;
use digua_http_client::models::TokenInfo;
use digua_http_client::options::KudiHttpOptions;
use digua_http_client::transport::{HttpResponse, HttpTransport};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Default)]
struct CountingTransport {
    token_calls: AtomicUsize,
}

#[async_trait]
impl HttpTransport for CountingTransport {
    async fn post_form(
        &self,
        _url: &str,
        _form: &HashMap<String, String>,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.token_calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: 200,
            body: serde_json::to_string(&TokenInfo {
                access_token: "svc-token".into(),
                refresh_token: None,
                token_type: "Bearer".into(),
                expires_in: 3600,
            })
            .unwrap(),
        })
    }

    async fn post_json(
        &self,
        _url: &str,
        _bearer: Option<&str>,
        _body: &str,
    ) -> Result<HttpResponse, KudiHttpError> {
        Ok(HttpResponse {
            status: 200,
            body: "{}".into(),
        })
    }

    async fn get(&self, _url: &str, _bearer: Option<&str>) -> Result<HttpResponse, KudiHttpError> {
        Ok(HttpResponse {
            status: 200,
            body: "{}".into(),
        })
    }
}

#[tokio::test]
async fn force_refresh_triggers_refetch() {
    let mock = Arc::new(CountingTransport::default());
    let client = KudiWebClient::with_transport(
        mock.clone(),
        KudiHttpOptions {
            token_endpoint: "http://t/token".into(),
            ..KudiHttpOptions::default()
        },
    )
    .with_credential("a", "s1");

    client.post("http://u/x", "{}", None).await.unwrap();
    client.post("http://u/y", "{}", None).await.unwrap();
    assert_eq!(mock.token_calls.load(Ordering::SeqCst), 1);

    client.force_refresh().await.unwrap();

    client.post("http://u/z", "{}", None).await.unwrap();
    assert_eq!(mock.token_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn force_refresh_is_noop_without_credential() {
    let mock = Arc::new(CountingTransport::default());
    let client = KudiWebClient::with_transport(mock.clone(), KudiHttpOptions::default());
    client.force_refresh().await.unwrap();
    assert_eq!(mock.token_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn base_url_round_trip() {
    let transport: Arc<dyn HttpTransport> = Arc::new(CountingTransport::default());

    let via_with_base = KudiWebClient::with_transport(transport, KudiHttpOptions::default())
        .with_base_url("https://gw.kudi.repos");
    assert_eq!(via_with_base.base_url(), Some("https://gw.kudi.repos"));

    let via_builder = KudiWebClient::builder()
        .base_url("https://gw.kudi.repos")
        .build()
        .unwrap();
    assert_eq!(via_builder.base_url(), Some("https://gw.kudi.repos"));

    let unset = KudiWebClient::builder().build().unwrap();
    assert_eq!(unset.base_url(), None);
}
