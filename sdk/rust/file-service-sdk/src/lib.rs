//! `file-service-sdk` — typed Rust client for the KudiCoder `file-service`.
//!
//! Endpoints from `services/file-service/openapi.yaml`. The SDK does not
//! proxy binary bytes — for upload/download, presign a URL and use any
//! HTTP client (or the SDK's underlying reqwest via
//! `kudi-http-client`) to PUT/GET directly against the storage URL.

pub mod client;
pub mod model;

pub use crate::client::FileClient;
pub use crate::model::{
    CreatePresignUrl, DownloadedFile, FileInfo, ListFilesQuery, PagedFiles, PresignUrlResponse,
    UploadFileMeta,
};
pub use digua_http_client::SdkError;
