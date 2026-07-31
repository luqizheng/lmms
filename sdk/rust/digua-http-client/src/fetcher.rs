use crate::error::KudiHttpError;
use crate::models::TokenInfo;
use crate::transport::HttpTransport;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ClientCredentialsFetcher {
    transport: Arc<dyn HttpTransport>,
    token_endpoint: String,
}

impl ClientCredentialsFetcher {
    pub fn new(transport: Arc<dyn HttpTransport>, token_endpoint: impl Into<String>) -> Self {
        Self {
            transport,
            token_endpoint: token_endpoint.into(),
        }
    }

    pub async fn fetch(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<TokenInfo, KudiHttpError> {
        let mut form: HashMap<String, String> = HashMap::new();
        form.insert("grant_type".into(), "client_credentials".into());
        form.insert("client_id".into(), client_id.into());
        form.insert("client_secret".into(), client_secret.into());

        let resp = self
            .transport
            .post_form(&self.token_endpoint, &form)
            .await?;

        if !(200..300).contains(&resp.status) {
            return Err(KudiHttpError::Auth {
                status: resp.status,
                message: format!("client_credentials failed: HTTP {}", resp.status),
                details: Some(resp.body),
            });
        }

        let info: TokenInfo = serde_json::from_str(&resp.body)
            .map_err(|e| KudiHttpError::Network(format!("parse token response: {}", e)))?;
        if info.access_token.is_empty() {
            return Err(KudiHttpError::Auth {
                status: resp.status,
                message: "token response missing access_token".into(),
                details: Some(resp.body),
            });
        }
        Ok(info)
    }
}
