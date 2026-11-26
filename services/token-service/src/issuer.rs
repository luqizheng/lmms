use jwt_simple::prelude::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Legacy hard-coded `kid` (pre-keystore era). Kept as the compatibility kid
/// assigned to keys imported from the old `keys/private.pem` pair so tokens
/// signed before the keystore migration keep verifying (their header says
/// `kid:"default"`). Newly generated keys use fingerprint-derived kids — see
/// `keystore::derive_kid` and the 2026-11-23 persistence design spec.
pub const JWT_KEY_ID: &str = crate::keystore::LEGACY_KID;

/// `purpose` claim: server-authoritative statement of WHY a token was
/// minted (its issuance channel). The value is decided exclusively by the
/// issuing code path — no request field can ever influence it (see
/// docs/superpowers/specs/2026-11-04-token-purpose-claim-design.md).
/// Closed enum; verifiers must treat unknown/missing values per contract.
pub mod purpose {
    /// Human password login (and iam-service refresh of such sessions).
    pub const USER_LOGIN: &str = "user_login";
    /// API Key client_credentials exchange (machine acting for a bound user).
    pub const API_KEY: &str = "api_key";
    /// OAuth authorization_code exchange (third-party client, consented).
    pub const AUTH_CODE: &str = "auth_code";
    /// Service-to-service bootstrap JWT (aud = target service).
    pub const SERVICE: &str = "service";
    /// Short-lived resource-scoped token.
    pub const RESOURCE: &str = "resource";
}

pub struct TokenIssuer {
    /// Swappable active signing key (std RwLock: sign is a short CPU-bound
    /// op). Runtime rotation replaces it via `swap_active_key` so every
    /// `Arc<TokenIssuer>` holder picks up the new key on the next issue call.
    key_pair: std::sync::RwLock<RS256KeyPair>,
    issuer: String,
    /// kid stamped into the JWT header by `sign` — must match the kid this
    /// key is published under in JWKS output.
    key_id: std::sync::RwLock<String>,
}

impl TokenIssuer {
    /// Legacy constructor: kid = `"default"` (compat with existing tests and
    /// the imported legacy key). Keystore-based startup uses `with_key`.
    pub fn new(private_key_pem: &str, issuer: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_key(private_key_pem, JWT_KEY_ID, issuer)
    }

    /// Build an issuer bound to a specific key + kid. The kid is stamped
    /// into every JWT header so verifiers can locate the matching JWKS key.
    /// jwt-simple's RSAKeyPairLike trait lacks `set_key_id` for
    /// RS256KeyPair — use the inherent builder `with_key_id`.
    pub fn with_key(
        private_key_pem: &str,
        key_id: &str,
        issuer: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let key_pair = RS256KeyPair::from_pem(private_key_pem)?.with_key_id(key_id);
        Ok(Self {
            key_pair: std::sync::RwLock::new(key_pair),
            issuer: issuer.to_string(),
            key_id: std::sync::RwLock::new(key_id.to_string()),
        })
    }

    /// The kid this issuer currently stamps into JWT headers.
    pub fn key_id(&self) -> String {
        self.key_id
            .read()
            .expect("issuer key_id lock poisoned")
            .clone()
    }

    /// Runtime rotation: atomically replace the signing key + kid. Tokens
    /// already issued keep verifying against the retiring key (still in
    /// JWKS); new tokens carry the new kid.
    pub fn swap_active_key(
        &self,
        private_key_pem: &str,
        key_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pair = RS256KeyPair::from_pem(private_key_pem)?.with_key_id(key_id);
        *self.key_pair.write().expect("issuer key lock poisoned") = pair;
        *self.key_id.write().expect("issuer key_id lock poisoned") = key_id.to_string();
        Ok(())
    }

    fn sign_current(
        &self,
        claims: JWTClaims<HashMap<String, serde_json::Value>>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let pair = self.key_pair.read().expect("issuer key lock poisoned");
        Ok(pair.sign(claims)?)
    }

    fn now() -> jwt_simple::prelude::Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        jwt_simple::prelude::Duration::from_secs(now)
    }

    pub fn issue_access_token(
        &self,
        sub: &str,
        client_id: &str,
        purpose: Option<&str>,
        extra_claims: serde_json::Value,
        ttl_sec: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some(sub.to_string());
        claims.jwt_id = Some(Uuid::new_v4().to_string());
        claims.issuer = Some(self.issuer.clone());
        claims.expires_at = Some(Self::now() + Duration::from_secs(ttl_sec as u64));

        let mut custom = HashMap::new();
        // Merge business claims FIRST, then stamp issuer-authoritative keys
        // (last-write-wins) so no extra claim can ever override them.
        if let Some(obj) = extra_claims.as_object() {
            for (k, v) in obj {
                custom.insert(k.clone(), v.clone());
            }
        }
        custom.insert(
            "scope".to_string(),
            serde_json::Value::String("access".to_string()),
        );
        custom.insert(
            "client_id".to_string(),
            serde_json::Value::String(client_id.to_string()),
        );
        if let Some(p) = purpose {
            custom.insert(
                "purpose".to_string(),
                serde_json::Value::String(p.to_string()),
            );
        }
        claims.custom = custom;

        let token = self.sign_current(claims)?;
        Ok(token)
    }

    /// `tid` is mandatory (WP-LMMS-REFRESH-TID-01): every refresh token is
    /// bound to the verified tenant at issuance and carries it through
    /// rotation. Refresh-side decode rejects tokens without it — no default
    /// tenant fallback, no legacy compatibility path.
    pub fn issue_refresh_token(
        &self,
        sub: &str,
        tid: i64,
        purpose: Option<&str>,
        ttl_sec: i64,
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        let family = Uuid::new_v4().to_string();
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some(sub.to_string());
        claims.jwt_id = Some(Uuid::new_v4().to_string());
        claims.issuer = Some(self.issuer.clone());
        claims.expires_at = Some(Self::now() + Duration::from_secs(ttl_sec as u64));

        let mut custom = HashMap::new();
        custom.insert(
            "scope".to_string(),
            serde_json::Value::String("refresh".to_string()),
        );
        custom.insert("tid".to_string(), serde_json::Value::from(tid));
        custom.insert(
            "family".to_string(),
            serde_json::Value::String(family.clone()),
        );
        // Carry the channel through refresh so the rotated access token can
        // inherit it (see routes/refresh.rs). Legacy tokens without it keep
        // producing purpose-less access tokens (contract: absent = legacy).
        if let Some(p) = purpose {
            custom.insert(
                "purpose".to_string(),
                serde_json::Value::String(p.to_string()),
            );
        }
        claims.custom = custom;

        let token = self.sign_current(claims)?;
        Ok((token, family))
    }

    pub fn issue_user_jwt(
        &self,
        user_id: &str,
        user_code: &str,
        extra_claims: serde_json::Value,
        ttl_secs: u64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some(user_id.to_string());
        claims.jwt_id = Some(Uuid::new_v4().to_string());
        claims.issuer = Some(self.issuer.clone());
        claims.audiences = Some(Audiences::AsString("coder-micro-services".to_string()));
        claims.expires_at = Some(Self::now() + Duration::from_secs(ttl_secs));

        let mut custom = HashMap::new();
        custom.insert(
            "scope".to_string(),
            serde_json::Value::String("access".to_string()),
        );
        custom.insert(
            "client_id".to_string(),
            serde_json::Value::String("credential-login-service".to_string()),
        );
        custom.insert(
            "user_code".to_string(),
            serde_json::Value::String(user_code.to_string()),
        );

        let reserved = ["iss", "aud", "exp", "iat", "nbf", "sub", "jti", "purpose"];
        if let Some(obj) = extra_claims.as_object() {
            for (k, v) in obj {
                if reserved.contains(&k.as_str()) {
                    continue;
                }
                custom.insert(k.clone(), v.clone());
            }
        }
        // Re-stamp authoritative keys AFTER the merge (last-write-wins) and
        // fix the channel to the login path — even a trusted internal caller
        // cannot mint a login token carrying another channel's purpose.
        custom.insert(
            "scope".to_string(),
            serde_json::Value::String("access".to_string()),
        );
        custom.insert(
            "client_id".to_string(),
            serde_json::Value::String("credential-login-service".to_string()),
        );
        custom.insert("user_code".to_string(), user_code.into());
        custom.insert(
            "purpose".to_string(),
            serde_json::Value::String(purpose::USER_LOGIN.to_string()),
        );
        claims.custom = custom;

        self.sign_current(claims)
    }

    pub fn issue_resource_token(
        &self,
        sub: &str,
        resource_type: &str,
        resource_id: &str,
        permission: &str,
        ttl_sec: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some(sub.to_string());
        claims.jwt_id = Some(Uuid::new_v4().to_string());
        claims.issuer = Some(self.issuer.clone());
        claims.expires_at = Some(Self::now() + Duration::from_secs(ttl_sec as u64));

        let mut custom = HashMap::new();
        custom.insert(
            "scope".to_string(),
            serde_json::Value::String("resource".to_string()),
        );
        custom.insert(
            "resource_type".to_string(),
            serde_json::Value::String(resource_type.to_string()),
        );
        custom.insert(
            "resource_id".to_string(),
            serde_json::Value::String(resource_id.to_string()),
        );
        custom.insert(
            "permission".to_string(),
            serde_json::Value::String(permission.to_string()),
        );
        custom.insert(
            "purpose".to_string(),
            serde_json::Value::String(purpose::RESOURCE.to_string()),
        );
        claims.custom = custom;

        let token = self.sign_current(claims)?;
        Ok(token)
    }
}
