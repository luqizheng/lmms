use common_error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEvent {
    pub action: String,
    pub resource: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub request_id: String,
    pub user_id: Option<String>,
    pub timestamp: u64,
}

pub struct AuditClient {
    endpoint: Option<String>,
    buffer: Arc<Mutex<VecDeque<AuditEvent>>>,
    buffer_path: Option<String>,
}

impl AuditClient {
    pub fn new(endpoint: Option<&str>, buffer_path: Option<&str>) -> Self {
        Self {
            endpoint: endpoint.map(|s| s.to_string()),
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            buffer_path: buffer_path.map(|s| s.to_string()),
        }
    }

    pub async fn log(
        &self,
        action: &str,
        resource: &str,
        before: Option<&str>,
        after: Option<&str>,
        request_id: &str,
        user_id: Option<&str>,
    ) -> AppResult<()> {
        let event = AuditEvent {
            action: action.to_string(),
            resource: resource.to_string(),
            before: before.map(|s| s.to_string()),
            after: after.map(|s| s.to_string()),
            request_id: request_id.to_string(),
            user_id: user_id.map(|s| s.to_string()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };

        if let Some(endpoint) = &self.endpoint {
            if let Err(e) = self.send_to_server(&event, endpoint).await {
                tracing::warn!("Failed to send audit event, buffering: {}", e);
                self.buffer_event(event).await?;
            }
        } else {
            self.buffer_event(event).await?;
        }

        Ok(())
    }

    async fn send_to_server(&self, event: &AuditEvent, endpoint: &str) -> AppResult<()> {
        let client = reqwest::Client::new();
        let res = client
            .post(endpoint)
            .json(event)
            .send()
            .await
            .map_err(|e| AppError::external("AUDIT_HTTP", e.to_string()))?;

        if !res.status().is_success() {
            return Err(AppError::external("AUDIT_RESPONSE", format!("{}", res.status())));
        }
        Ok(())
    }

    async fn buffer_event(&self, event: AuditEvent) -> AppResult<()> {
        let mut buffer = self.buffer.lock().await;
        buffer.push_back(event);

        if buffer.len() >= 100 {
            if let Some(path) = &self.buffer_path {
                self.flush_buffer_to_disk(path).await?;
            }
        }
        Ok(())
    }

    async fn flush_buffer_to_disk(&self, path: &str) -> AppResult<()> {
        let mut buffer = self.buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let file_path = Path::new(path);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .map_err(|e| AppError::internal("AUDIT_FILE", e.to_string()))?;

        while let Some(event) = buffer.pop_front() {
            let line = serde_json::to_string(&event)
                .map_err(|e| AppError::internal("AUDIT_SERIALIZE", e.to_string()))?;
            writeln!(file, "{}", line)
                .map_err(|e| AppError::internal("AUDIT_WRITE", e.to_string()))?;
        }

        Ok(())
    }
}