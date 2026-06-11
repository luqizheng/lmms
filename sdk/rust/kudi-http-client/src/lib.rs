//! Kudi HTTP client SDK: service-to-service (client_credentials) + user Bearer passthrough.

// Install the rustls crypto provider once. Safe to call multiple times.
use std::sync::Once;
static CRYPTO_INIT: Once = Once::new();
fn ensure_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

pub mod options;
pub mod cache;
pub mod fetcher;
pub mod transport;
pub mod error;
pub mod models;
pub mod client;
