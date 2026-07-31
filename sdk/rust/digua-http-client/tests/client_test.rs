use async_trait::async_trait;
use digua_http_client::client::KudiWebClient;
use digua_http_client::error::KudiHttpError;
use digua_http_client::models::TokenInfo;
use digua_http_client::options::KudiHttpOptions;
use digua_http_client::transport::{HttpResponse, HttpTransport};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockTransport {
    token_calls: AtomicUsize,
    downstream_calls: AtomicUsize,
    last_authorization: Mutex<Option<String>>,
}

#[async_trait]
impl HttpTransport for MockTransport {
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
        bearer: Option<&str>,
        _json_body: &str,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.downstream_calls.fetch_add(1, Ordering::SeqCst);
        *self.last_authorization.lock().unwrap() = bearer.map(|s| format!("Bearer {}", s));
        Ok(HttpResponse {
            status: 200,
            body: "{}".into(),
        })
    }

    async fn get(&self, _url: &str, bearer: Option<&str>) -> Result<HttpResponse, KudiHttpError> {
        self.downstream_calls.fetch_add(1, Ordering::SeqCst);
        *self.last_authorization.lock().unwrap() = bearer.map(|s| format!("Bearer {}", s));
        Ok(HttpResponse {
            status: 200,
            body: "{}".into(),
        })
    }
}

#[tokio::test]
async fn fetches_token_once_and_uses_for_subsequent_calls() {
    let mock = Arc::new(MockTransport::default());
    let client = KudiWebClient::with_transport(
        mock.clone(),
        KudiHttpOptions {
            token_endpoint: "http://t/token".into(),
            ..KudiHttpOptions::default()
        },
    )
    .with_credential("a", "s1");

    client.get("http://u/api/x", None).await.unwrap();
    client.get("http://u/api/y", None).await.unwrap();
    client.post("http://u/api/z", "{}", None).await.unwrap();

    assert_eq!(mock.token_calls.load(Ordering::SeqCst), 1);
    assert_eq!(mock.downstream_calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        mock.last_authorization.lock().unwrap().as_deref(),
        Some("Bearer svc-token")
    );
}

#[tokio::test]
async fn user_bearer_passthrough_skips_token_fetch() {
    let mock = Arc::new(MockTransport::default());
    let client = KudiWebClient::with_transport(
        mock.clone(),
        KudiHttpOptions {
            token_endpoint: "http://t/token".into(),
            ..KudiHttpOptions::default()
        },
    )
    .with_credential("a", "s1");

    client
        .get_with_bearer("http://u/api/me", "user.jwt")
        .await
        .unwrap();
    assert_eq!(mock.token_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        mock.last_authorization.lock().unwrap().as_deref(),
        Some("Bearer user.jwt")
    );
}
