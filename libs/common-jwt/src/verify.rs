use jsonwebtoken::{decode, decode_header, encode, Algorithm, EncodingKey, Header, Validation};
use serde_json::{json, Value};
use std::time::Duration;
use thiserror::Error;

use crate::cache::{JWKSCache, JwksError};

#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("malformed JWT: {0}")]
    Malformed(String),
    #[error("signature invalid")]
    SignatureInvalid,
    #[error("audience mismatch: expected {expected}, got {got}")]
    AudienceMismatch { expected: String, got: String },
    #[error("expired")]
    Expired,
    #[error("JWKS error: {0}")]
    Jwks(#[from] JwksError),
}

#[derive(Error, Debug)]
pub enum SignError {
    #[error("invalid PEM key: {0}")]
    InvalidKey(String),
    #[error("encode failed: {0}")]
    Encode(String),
}

/// Sign a JWT with RS256 and the given PEM-encoded private key.
/// `claims["iat"]` and `claims["exp"]` are overridden based on `now() + ttl`.
pub fn sign_token(
    private_key_pem: &str,
    claims: &Value,
    ttl: Duration,
) -> Result<String, SignError> {
    let mut claims = claims.clone();
    let now = chrono::Utc::now().timestamp();
    claims["iat"] = json!(now);
    claims["exp"] = json!(now + ttl.as_secs() as i64);
    let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(|e| SignError::InvalidKey(e.to_string()))?;
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("default".to_string());
    encode(&header, &claims, &key).map_err(|e| SignError::Encode(e.to_string()))
}

/// Verify a JWT against a JWKSCache. If `expected_aud` is Some, the JWT's
/// `aud` claim must equal `expected_aud` exactly.
pub async fn verify_token(
    token: &str,
    cache: &JWKSCache,
    expected_aud: Option<&str>,
) -> Result<Value, VerifyError> {
    let header = decode_header(token).map_err(|e| VerifyError::Malformed(e.to_string()))?;
    // Phase 5-B dev convenience: if kid is missing or unknown, fall
    // back to the default key. Production hardening (kid strict match)
    // can be enforced by routing only the "service" audience through
    // this path while user-jwt (aud=user) gets a kid-aware verifier.
    let key = match header.kid.as_deref() {
        Some(kid) => cache
            .get(kid)
            .or_else(|| cache.default_key())
            .ok_or_else(|| VerifyError::Malformed(format!("unknown kid: {kid}")))?,
        None => cache
            .default_key()
            .ok_or_else(|| VerifyError::Malformed("no default key in cache".into()))?,
    };
    let mut validation = Validation::new(Algorithm::RS256);
    let expected_aud_owned: String;
    match expected_aud {
        Some(aud) => {
            expected_aud_owned = aud.to_string();
            validation.set_audience(&[&expected_aud_owned]);
        }
        None => {
            validation.validate_aud = false;
        }
    }
    let data = decode::<Value>(token, &key, &validation).map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::InvalidSignature => VerifyError::SignatureInvalid,
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => VerifyError::Expired,
        jsonwebtoken::errors::ErrorKind::InvalidAudience => VerifyError::AudienceMismatch {
            expected: expected_aud.unwrap_or("(none)").to_string(),
            got: "(see token)".to_string(),
        },
        _ => VerifyError::Malformed(e.to_string()),
    })?;
    Ok(data.claims)
}
