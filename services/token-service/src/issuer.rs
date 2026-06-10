use jwt_simple::prelude::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub struct TokenIssuer {
    key_pair: RS256KeyPair,
    issuer: String,
}

impl TokenIssuer {
    pub fn new(private_key_pem: &str, issuer: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let key_pair = RS256KeyPair::from_pem(private_key_pem)?;
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
        extra_claims: serde_json::Value,
        ttl_sec: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some(sub.to_string());
        claims.jwt_id = Some(Uuid::new_v4().to_string());
        claims.issuer = Some(self.issuer.clone());
        claims.expires_at = Some(Self::now() + Duration::from_secs(ttl_sec as u64));

        let mut custom = HashMap::new();
        custom.insert("scope".to_string(), serde_json::Value::String("access".to_string()));
        custom.insert("client_id".to_string(), serde_json::Value::String(client_id.to_string()));
        // 合并 user-service 提供的业务 claims，user-service 决定 schema
        if let Some(obj) = extra_claims.as_object() {
            for (k, v) in obj {
                custom.insert(k.clone(), v.clone());
            }
        }
        claims.custom = custom;

        let token = self.key_pair.sign(claims)?;
        Ok(token)
    }

    pub fn issue_refresh_token(
        &self,
        sub: &str,
        ttl_sec: i64,
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        let family = Uuid::new_v4().to_string();
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some(sub.to_string());
        claims.jwt_id = Some(Uuid::new_v4().to_string());
        claims.issuer = Some(self.issuer.clone());
        claims.expires_at = Some(Self::now() + Duration::from_secs(ttl_sec as u64));

        let mut custom = HashMap::new();
        custom.insert("scope".to_string(), serde_json::Value::String("refresh".to_string()));
        custom.insert("family".to_string(), serde_json::Value::String(family.clone()));
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
        use std::collections::HashSet;
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some(user_id.to_string());
        claims.jwt_id = Some(Uuid::new_v4().to_string());
        claims.issuer = Some(self.issuer.clone());
        claims.audiences = Some(HashSet::from(["coder-micro-services".to_string()]));
        claims.expires_at = Some(Self::now() + Duration::from_secs(ttl_secs));

        let mut custom = HashMap::new();
        custom.insert("scope".to_string(), serde_json::Value::String("access".to_string()));
        custom.insert("client_id".to_string(), serde_json::Value::String("credential-login-service".to_string()));
        custom.insert("user_code".to_string(), serde_json::Value::String(user_code.to_string()));

        let reserved = ["iss", "aud", "exp", "iat", "nbf", "sub", "jti"];
        if let Some(obj) = extra_claims.as_object() {
            for (k, v) in obj {
                if reserved.contains(&k.as_str()) {
                    continue;
                }
                custom.insert(k.clone(), v.clone());
            }
        }
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
        custom.insert("scope".to_string(), serde_json::Value::String("resource".to_string()));
        custom.insert("resource_type".to_string(), serde_json::Value::String(resource_type.to_string()));
        custom.insert("resource_id".to_string(), serde_json::Value::String(resource_id.to_string()));
        custom.insert("permission".to_string(), serde_json::Value::String(permission.to_string()));
        claims.custom = custom;

        let token = self.key_pair.sign(claims)?;
        Ok(token)
    }
}
