use common_error::{AppError, AppResult};
use std::collections::HashMap;

pub struct ConfigServiceProvider {
    base_url: String,
    namespace_id: String,
    client: reqwest::Client,
}

impl ConfigServiceProvider {
    pub fn new(base_url: impl Into<String>, namespace_id: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url: base_url.into(),
            namespace_id: namespace_id.into(),
            client,
        }
    }

    pub async fn fetch(&self) -> AppResult<HashMap<String, String>> {
        let url = format!("{}/config/export/{}", self.base_url, self.namespace_id);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::external("CFG_PROVIDER_HTTP", e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AppError::external(
                "CFG_PROVIDER_STATUS",
                format!("config-service returned {}", resp.status()),
            ));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| AppError::external("CFG_PROVIDER_BODY", e.to_string()))?;

        let map: HashMap<String, String> = serde_json::from_str(&body)
            .map_err(|e| AppError::external("CFG_PROVIDER_PARSE", e.to_string()))?;

        Ok(map)
    }
}
