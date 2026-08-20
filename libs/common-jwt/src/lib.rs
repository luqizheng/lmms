//! digua_common-jwt: shared JWT primitives (JWKS cache, verify, sign, service-jwt cache)
//!
//! Phase 5-B. Replaces ad-hoc JWT handling scattered across services.

pub mod cache;
pub mod service_jwt;
pub mod verify;

pub use cache::{parse_jwks, JWKSCache, JwksConfig, JwksError};
pub use service_jwt::{ServiceJwtCache, ServiceJwtConfig, ServiceJwtError};
pub use verify::{sign_token, verify_token, SignError, VerifyError};
