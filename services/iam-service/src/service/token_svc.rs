use crate::{
    state::AppState,
    error::IamError,
};
use jwt_simple::prelude::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TokenService {
    state: AppState,
    key_pair: RS256KeyPair,
    public_key: RS256PublicKey,
}

impl TokenService {
    pub fn new(state: AppState) -> Result<Self, IamError> {
        let key_pair = Self::load_key_pair(&state.config.jwt)?;
        let public_key = key_pair.public_key();
        Ok(Self { state, key_pair, public_key })
    }

    fn load_key_pair(config: &crate::config::JwtConfig) -> Result<RS256KeyPair, IamError> {
        let private_key_pem = std::fs::read_to_string(&config.private_key_path)
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        let key_pair = RS256KeyPair::from_pem(&private_key_pem)
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        Ok(key_pair)
    }

    fn now() -> Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Duration::from_secs(now)
    }

    pub fn issue_access_token(&self, user_id: &str, username: &str, tenant_id: i64, roles: Vec<String>, permissions: Vec<String>) -> Result<String, IamError> {
        let ttl_sec = self.state.config.jwt.access_token_ttl_minutes * 60;
        
        let now = Self::now();
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some(user_id.to_string());
        claims.issuer = Some("superpowers-token-service".to_string());
        claims.issued_at = Some(now);
        claims.jwt_id = Some(uuid::Uuid::new_v4().to_string());
        claims.expires_at = Some(now + Duration::from_secs(ttl_sec as u64));

        let mut custom = HashMap::new();
        custom.insert("username".to_string(), serde_json::Value::String(username.to_string()));
        custom.insert("tid".to_string(), serde_json::Value::Number(serde_json::Number::from(tenant_id)));
        custom.insert("roles".to_string(), serde_json::Value::Array(roles.into_iter().map(|r| serde_json::Value::String(r)).collect()));
        custom.insert("permissions".to_string(), serde_json::Value::Array(permissions.into_iter().map(|p| serde_json::Value::String(p)).collect()));
        claims.custom = custom;

        let token = self.key_pair.sign(claims)
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        Ok(token)
    }

    pub fn verify_token(&self, token: &str) -> Result<JWTClaims<HashMap<String, serde_json::Value>>, IamError> {
        let claims = self.public_key.verify_token::<HashMap<String, serde_json::Value>>(token, None)
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        Ok(claims)
    }
}
