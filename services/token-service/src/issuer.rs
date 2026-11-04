use jwt_simple::prelude::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Phase 5-B: the `kid` JWT header field. Must match the key id in
/// the JWKSCache on the verifier side (see `digua_common_jwt::JWKSCache::from_pem`).
pub const JWT_KEY_ID: &str = "default";

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
    key_pair: RS256KeyPair,
    issuer: String,
}

impl TokenIssuer {
    pub fn new(private_key_pem: &str, issuer: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Phase 5-B: set kid so verifiers can locate the JWKSCache key.
        // jwt-simple 0.12.16's RSAKeyPairLike trait does not implement
        // `set_key_id` for RS256KeyPair (only RS256PublicKey). Use the
        // inherent builder `with_key_id` instead.
        let key_pair = RS256KeyPair::from_pem(private_key_pem)?.with_key_id(JWT_KEY_ID);
        Ok(Self {
            key_pair,
            issuer: issuer.to_string(),
        })
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

        let token = self.key_pair.sign(claims)?;
        Ok(token)
    }

    pub fn issue_refresh_token(
        &self,
        sub: &str,
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

        let token = self.key_pair.sign(claims)?;
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

        Ok(self.key_pair.sign(claims)?)
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

        let token = self.key_pair.sign(claims)?;
        Ok(token)
    }
}
