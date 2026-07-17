//! `notification-service-sdk` — typed Rust client for the KudiCoder
//! `notification-service`. Endpoints from
//! `services/notification-service/openapi.yaml`.
//!
//! WebSocket subscription (`/notification/ws`) is intentionally not
//! wrapped here — WS streaming sits outside the typed request/response
//! model and the existing `kudi-http-client` is HTTP-only. Use any
//! tungstenite-based client and the URL below.

pub mod client;
pub mod model;

pub use crate::client::NotificationClient;
pub use crate::model::{
    Announcement, CreateAnnouncement, ListAnnouncementsQuery, PagedAnnouncements,
};
pub use kudi_http_client::SdkError;