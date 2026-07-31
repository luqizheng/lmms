//! `config-service-sdk` — typed Rust client for the KudiCoder
//! `config-service`. Endpoints from `services/config-service/openapi.yaml`.
//!
//! SSE watch (`GET /config/watch`) is intentionally not wrapped; use any
//! SSE-aware client (e.g. `eventsource-client`) to consume change events.

pub mod client;
pub mod model;

pub use crate::client::ConfigClient;
pub use crate::model::{
    ConfigItem, CreateConfigItem, CreateNamespace, ListItemsQuery, ListNamespacesQuery,
    Namespace, PagedConfigItems, PagedNamespaces,
};
pub use digua_http_client::SdkError;