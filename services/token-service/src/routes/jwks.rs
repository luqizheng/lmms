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

/// JWKS documents every key that verifiers may still need: `active` (current
/// signer) + `retiring` (verify-only after rotation). `retired` keys are NOT
/// published — removing them from the JWKS is exactly how verifiers stop
/// accepting tokens signed by them.
pub async fn handle_jwks(
    State(state): State<TokenServerState>,
) -> Result<Json<JwksResponse>, (StatusCode, &'static str)> {
    let pub_pems = {
        let ks = state.keys.read().expect("keystore lock poisoned");
        ks.public_entries()
            .into_iter()
            .map(|k| (k.kid.clone(), k.public_pem.clone()))
            .collect::<Vec<_>>()
    };
    Ok(Json(build_jwks(&pub_pems)?))
}

/// Pure key→JWK conversion (unit-testable without app state).
fn build_jwks(entries: &[(String, String)]) -> Result<JwksResponse, (StatusCode, &'static str)> {
    use rsa::pkcs1::DecodeRsaPublicKey;
    use rsa::traits::PublicKeyParts;
    let mut keys = Vec::with_capacity(entries.len());
    for (kid, public_key_pem) in entries {
        let key = rsa::RsaPublicKey::from_pkcs1_pem(public_key_pem)
            .inspect_err(|&e| {
                tracing::debug!("pkcs1 decode failed: {:?}, trying pkcs8...", e);
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

        keys.push(Jwk {
            kty: "RSA".into(),
            alg: "RS256".into(),
            n: n_b64,
            e: e_b64,
            kid: kid.clone(),
            use_: "sig".into(),
        });
    }
    Ok(JwksResponse { keys })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystore::{KeyStore, LEGACY_KID};
    use jwt_simple::prelude::*;
    use rsa::pkcs8::DecodePublicKey;

    fn pair() -> (RS256KeyPair, String) {
        let kp = RS256KeyPair::generate(2048).unwrap();
        let pub_pem = kp.public_key().to_pem().unwrap();
        (kp, pub_pem)
    }

    #[test]
    fn build_jwks_single_key_carries_kid_and_components() {
        let (_kp, pub_pem) = pair();
        let kid = crate::keystore::derive_kid(&pub_pem).unwrap();
        let resp = build_jwks(&[(kid.clone(), pub_pem)]).unwrap();
        assert_eq!(resp.keys.len(), 1);
        assert_eq!(resp.keys[0].kid, kid);
        assert_ne!(resp.keys[0].kid, LEGACY_KID, "fresh kid is fingerprint");
        assert_eq!(resp.keys[0].alg, "RS256");
        assert_eq!(resp.keys[0].kty, "RSA");
        assert_eq!(resp.keys[0].use_, "sig");
        // n/e must be usable by a JWKS consumer to rebuild the key
        let n = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&resp.keys[0].n)
            .unwrap();
        assert!(n.len() == 256, "RSA-2048 modulus is 256 bytes");
        let e = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&resp.keys[0].e)
            .unwrap();
        assert_eq!(e, vec![0x01, 0x00, 0x01], "standard RSA exponent");
    }

    // 任务 5/6：JWKS 返回正确公钥；轮换后新旧 kid 都在
    #[test]
    fn build_jwks_lists_active_and_retiring_kids() {
        let dir = tempfile::tempdir().unwrap();
        let mut ks = KeyStore::init(&dir.path().join("keystore.json"), "np", "nq", 100).unwrap();
        let (new_kid, prev_kid) = ks
            .rotate(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            )
            .unwrap();
        let entries: Vec<(String, String)> = ks
            .public_entries()
            .into_iter()
            .map(|k| (k.kid.clone(), k.public_pem.clone()))
            .collect();
        let resp = build_jwks(&entries).unwrap();
        let kids: Vec<&str> = resp.keys.iter().map(|k| k.kid.as_str()).collect();
        assert_eq!(kids.len(), 2);
        assert!(kids.contains(&new_kid.as_str()), "active kid in JWKS");
        assert!(kids.contains(&prev_kid.as_str()), "retiring kid in JWKS");
        // each published n matches its own key
        for k in &resp.keys {
            let pem = entries.iter().find(|(kid, _)| *kid == k.kid).unwrap();
            let key = rsa::RsaPublicKey::from_public_key_pem(&pem.1).unwrap();
            use rsa::traits::PublicKeyParts;
            let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.n().to_bytes_be());
            assert_eq!(n, k.n);
        }
    }

    #[test]
    fn build_jwks_rejects_garbage_pem() {
        assert!(build_jwks(&[("k".into(), "garbage".into())]).is_err());
    }
}
