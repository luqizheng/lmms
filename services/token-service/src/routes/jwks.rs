use crate::TokenServerState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use base64::Engine;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Jwk {
    pub kty: String,
    pub alg: String,
    pub n: String,
    pub e: String,
    pub kid: String,
    pub use_: String,
}

#[derive(Debug, Serialize)]
pub struct JwksResponse {
    pub keys: Vec<Jwk>,
}

pub async fn handle_jwks(
    State(state): State<TokenServerState>,
) -> Result<Json<JwksResponse>, (StatusCode, &'static str)> {
    let public_key_pem = &state.jwt_public_key;
    use rsa::pkcs1::DecodeRsaPublicKey;
    use rsa::traits::PublicKeyParts;
    let key = rsa::RsaPublicKey::from_pkcs1_pem(public_key_pem)
        .inspect_err(|&e| {
            tracing::warn!("pkcs1 decode failed: {:?}, trying pkcs8...", e);
        })
        .or_else(|_| {
            use rsa::pkcs8::DecodePublicKey;
            rsa::RsaPublicKey::from_public_key_pem(public_key_pem).map_err(|e| {
                tracing::error!("pkcs8 decode also failed: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "invalid public key")
            })
        })?;

    let components = key.n().to_bytes_be();
    let n_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&components);

    let e_components = key.e().to_bytes_be();
    let e_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&e_components);

    let kid = uuid::Uuid::new_v4().to_string();

    Ok(Json(JwksResponse {
        keys: vec![Jwk {
            kty: "RSA".into(),
            alg: "RS256".into(),
            n: n_b64,
            e: e_b64,
            kid,
            use_: "sig".into(),
        }],
    }))
}
