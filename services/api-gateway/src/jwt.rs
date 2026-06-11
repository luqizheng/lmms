use axum::http::StatusCode;
use jwt_simple::prelude::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct JwtClaims {
    pub sub: String,
    pub jti: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
    pub scope: String,
    pub client_id: String,
    pub permissions: Vec<String>,
    /// 透传 user-service 提供的业务 claims。键名由 user-service 决定。
    pub custom: HashMap<String, serde_json::Value>,
}

fn extract_claims(jwt_claims: &JWTClaims<HashMap<String, serde_json::Value>>) -> JwtClaims {
    let sub = jwt_claims.subject.clone().unwrap_or_default();
    let jti = jwt_claims.jwt_id.clone().unwrap_or_default();
    let iss = jwt_claims.issuer.clone().unwrap_or_default();
    let iat = jwt_claims.issued_at.map(|d| d.as_secs() as i64).unwrap_or(0);
    let exp = jwt_claims.expires_at.map(|d| d.as_secs() as i64).unwrap_or(0);

    let scope = jwt_claims
        .custom
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let client_id = jwt_claims
        .custom
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let permissions: Vec<String> = jwt_claims
        .custom
        .get("permissions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    JwtClaims {
        sub,
        jti,
        iss,
        iat,
        exp,
        scope,
        client_id,
        permissions,
        custom: jwt_claims.custom.clone(),
    }
}

pub struct JwtValidator {
    public_key: RwLock<RS256PublicKey>,
    issuer: String,
}

impl JwtValidator {
    pub fn new(public_key_pem: &str, issuer: &str) -> Result<Self, StatusCode> {
        let public_key =
            RS256PublicKey::from_pem(public_key_pem).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Self {
            public_key: RwLock::new(public_key),
            issuer: issuer.to_string(),
        })
    }

    pub fn from_key(public_key: RS256PublicKey, issuer: &str) -> Result<Self, StatusCode> {
        Ok(Self {
            public_key: RwLock::new(public_key),
            issuer: issuer.to_string(),
        })
    }

    pub fn update(&self, key: RS256PublicKey) {
        if let Ok(mut p) = self.public_key.write() {
            *p = key;
        }
    }

    pub fn validate(&self, token: &str) -> Result<JwtClaims, StatusCode> {
        let key = self
            .public_key
            .read()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let mut options = VerificationOptions::default();
        options.allowed_issuers = Some(HashSet::from([self.issuer.clone()]));
        options.accept_future = false;

        let claims = key
            .verify_token::<HashMap<String, serde_json::Value>>(token, Some(options))
            .map_err(|e| {
                tracing::warn!("jwt validation failed: {:?}", e);
                StatusCode::UNAUTHORIZED
            })?;

        Ok(extract_claims(&claims))
    }
}
