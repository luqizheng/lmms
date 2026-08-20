//! `token-service-sdk` — typed Rust client for the KudiCoder `token-service`.
//!
//! Endpoints from `services/token-service/openapi.yaml`. The SDK does not
//! own credentials — it talks to `token-service` over its public API, and
//! the caller supplies the bearer JWT (typically the one issued by
//! `iam-service` after password login).

pub mod client;
pub mod model;

pub use crate::client::TokenClient;
pub use crate::model::{
    AccessRequest, AccessResponse, ApiKeyCreated, ApiKeySummary, CreateApiKeyRequest,
    ListApiKeysQuery, ListApiKeysResponse, RefreshRequest, RefreshResponse, RevokeRequest,
    RevokeResponse,
};
pub use digua_http_client::SdkError;
