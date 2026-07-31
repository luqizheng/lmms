use digua_common_jwt::{ServiceJwtCache, ServiceJwtError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Serialize)]
struct UserJwtRequest {
    user_id: String,
    user_code: String,
    claims: serde_json::Value,
    ttl_secs: u64,
}

#[derive(Debug, Deserialize)]
struct UserJwtResponse {
    access_token: String,
    #[allow(dead_code)]
    expires_in: u64,
}

#[derive(Error, Debug)]
pub enum TokenForwardError {
    #[error("service-jwt mint failed: {0}")]
    ServiceJwt(#[from] ServiceJwtError),
    #[error("token-service http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("token-service rejected request: status={0}")]
    Rejected(u16),
}

/// Forwards user-JWT issuance to the token-service over the service mesh.
/// The call is gated by a service-jwt with `aud=token-service` minted from
/// our bootstrap token, so only services holding a bootstrap token can mint.
pub struct TokenForwardClient {
    service_jwt: Arc<ServiceJwtCache>,
    token_service_url: String,
    http: reqwest::Client,
}

impl TokenForwardClient {
    pub fn new(service_jwt: Arc<ServiceJwtCache>, token_service_url: String) -> Self {
        Self {
            service_jwt,
            token_service_url,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }

    pub async fn issue_user_jwt(
        &self,
        user_id: &str,
        user_code: &str,
        claims: serde_json::Value,
        ttl_secs: u64,
    ) -> Result<String, TokenForwardError> {
        let service_jwt = self.service_jwt.get("token-service").await?;
        let url = format!(
            "{}/tokens/user-jwt",
            self.token_service_url.trim_end_matches('/')
        );
        let req = UserJwtRequest {
            user_id: user_id.to_string(),
            user_code: user_code.to_string(),
            claims,
            ttl_secs,
        };
        let resp = self
            .http
            .post(&url)
            .timeout(Duration::from_secs(5))
            .bearer_auth(&service_jwt)
            .json(&req)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(TokenForwardError::Rejected(status.as_u16()));
        }
        let parsed: UserJwtResponse = resp.json().await?;
        Ok(parsed.access_token)
    }
}