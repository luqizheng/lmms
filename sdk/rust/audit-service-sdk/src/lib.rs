//! `audit-service-sdk` — typed Rust client for the KudiCoder `audit-service`.
//!
//! Endpoints are taken from `services/audit-service/openapi.yaml` (the
//! single source of truth; Java SDK paths are stale and have diverged).
//!
//! ```no_run
//! use digua_audit_sdk::{AuditClient, LoginLogQuery};
//!
//! let token = std::sync::Arc::new(std::sync::Mutex::new(Some("user.jwt".into())));
//! let client = AuditClient::new("https://gw.kudi.repos", {
//!     let token = token.clone();
//!     move || token.lock().unwrap().clone()
//! });
//!
//! let _query = LoginLogQuery::new().page(0).size(20);
//! let _client = client;
//! ```

pub mod client;
pub mod model;

pub use crate::client::AuditClient;
pub use crate::model::{
    DataStage, LoginLog, LoginLogQuery, OperationLog, OperationLogQuery, PagedLoginLogs,
    PagedOperationLogs, RetentionPolicy,
};
pub use digua_http_client::SdkError;