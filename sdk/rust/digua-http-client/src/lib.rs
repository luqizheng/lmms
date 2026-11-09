//! Kudi HTTP client SDK: service-to-service (client_credentials) + user Bearer passthrough.

// Install the rustls crypto provider once. Safe to call multiple times.
// 只在 tls-rustls 轴下存在：native-tls 消费方（下游业务仓）根本不带 rustls，
// 留着这段会把 aws_lc_rs 拖进依赖图，且 provider 本就无从安装。
#[cfg(feature = "tls-rustls")]
fn ensure_crypto_provider() {
    use std::sync::Once;
    static CRYPTO_INIT: Once = Once::new();
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

#[cfg(not(feature = "tls-rustls"))]
fn ensure_crypto_provider() {}

pub mod cache;
pub mod client;
pub mod error;
pub mod fetcher;
pub mod models;
pub mod options;
pub mod transport;

/// Re-export of [`error::KudiHttpError`] under the canonical SDK name.
///
/// Service SDKs (iam/token/audit/...) should `use digua_http_client::SdkError`
/// for the error type so callers don't have to know the internal name.
/// Aligns with the multi-language SDK design spec
/// `docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md` §9.
pub use crate::error::KudiHttpError as SdkError;
