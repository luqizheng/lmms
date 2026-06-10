use base64::Engine;
use jwt_simple::prelude::*;
use rsa::pkcs8::{EncodePublicKey, LineEnding};
use serde::Deserialize;
use std::time::Duration;

use crate::JwtValidator;

#[derive(Debug, Deserialize)]
struct Jwk {
    kty: String,
    alg: String,
    n: String,
    e: String,
    #[serde(default)]
    kid: String,
    #[serde(rename = "use", default)]
    use_: String,
}

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Debug)]
pub enum JwksError {
    Http(reqwest::Error),
    Empty,
    Unsupported { kty: String, alg: String },
    Base64(base64::DecodeError),
    Rsa(rsa::Error),
    Pem(String),
    Jwt(jwt_simple::Error),
}

impl std::fmt::Display for JwksError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwksError::Http(e) => write!(f, "jwks http error: {e}"),
            JwksError::Empty => write!(f, "jwks response missing `keys` array"),
            JwksError::Unsupported { kty, alg } => write!(f, "unsupported jwk: kty={kty}, alg={alg}"),
            JwksError::Base64(e) => write!(f, "base64 decode error: {e}"),
            JwksError::Rsa(e) => write!(f, "rsa key error: {e}"),
            JwksError::Pem(e) => write!(f, "pem encode error: {e}"),
            JwksError::Jwt(e) => write!(f, "jwt-simple error: {e}"),
        }
    }
}

impl std::error::Error for JwksError {}

impl From<reqwest::Error> for JwksError {
    fn from(e: reqwest::Error) -> Self {
        JwksError::Http(e)
    }
}
impl From<base64::DecodeError> for JwksError {
    fn from(e: base64::DecodeError) -> Self {
        JwksError::Base64(e)
    }
}
impl From<rsa::Error> for JwksError {
    fn from(e: rsa::Error) -> Self {
        JwksError::Rsa(e)
    }
}
impl From<jwt_simple::Error> for JwksError {
    fn from(e: jwt_simple::Error) -> Self {
        JwksError::Jwt(e)
    }
}

fn jwk_to_rs256(jwk: &Jwk) -> Result<RS256PublicKey, JwksError> {
    if jwk.kty != "RSA" || jwk.alg != "RS256" {
        return Err(JwksError::Unsupported {
            kty: jwk.kty.clone(),
            alg: jwk.alg.clone(),
        });
    }
    let n_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&jwk.n)?;
    let e_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&jwk.e)?;
    let n = rsa::BigUint::from_bytes_be(&n_bytes);
    let e = rsa::BigUint::from_bytes_be(&e_bytes);
    let key = rsa::RsaPublicKey::new(n, e)?;
    let pem = key
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| JwksError::Pem(e.to_string()))?;
    Ok(RS256PublicKey::from_pem(&pem)?)
}

pub async fn fetch_rs256_key(
    client: &reqwest::Client,
    url: &str,
) -> Result<RS256PublicKey, JwksError> {
    let resp = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<JwksResponse>()
        .await?;
    let jwk = resp.keys.into_iter().next().ok_or(JwksError::Empty)?;
    jwk_to_rs256(&jwk)
}

pub fn spawn_refresher(
    client: reqwest::Client,
    url: String,
    interval_secs: u64,
    validator: std::sync::Arc<JwtValidator>,
) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(interval_secs));
        tick.tick().await;
        loop {
            tick.tick().await;
            match fetch_rs256_key(&client, &url).await {
                Ok(key) => {
                    tracing::info!("jwks refreshed from {}", url);
                    validator.update(key);
                }
                Err(e) => {
                    tracing::warn!("jwks refresh failed (url={}): {e}", url);
                }
            }
        }
    });
}
