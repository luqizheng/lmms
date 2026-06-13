use crate::{
    error::IamError,
    models::user::UserStatus,
    models::User,
    state::AppState,
    dto::{LoginRequest, LoginResponse, ChangePasswordRequest, UserInfo},
    service::TokenService,
};
use common_crypto::password::{hash, verify};
use chrono::Utc;
use nanoid::nanoid;

pub struct AuthService {
    state: AppState,
}

impl AuthService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse, IamError> {
        let user: Option<User> = sqlx::query_as(
            "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE username = ? AND deleted_at IS NULL"
        )
        .bind(&req.username)
        .fetch_optional(&self.state.db)
        .await?;

        let user = user.ok_or(IamError::InvalidCredentials)?;

        if user.status != UserStatus::Active {
            return Err(IamError::UserDisabled);
        }

        if let Some(expired_at) = user.password_expired_at {
            if expired_at < Utc::now() {
                return Err(IamError::PasswordExpired);
            }
        }

        let valid = verify(&req.password, &user.password_hash)
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        if !valid {
            return Err(IamError::InvalidCredentials);
        }

        let token_service = TokenService::new(self.state.clone())
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        let access_token = token_service.issue_access_token(&user.id, &user.username, vec![], vec![])
            .map_err(|e| IamError::Jwt(e.to_string()))?;

        sqlx::query(
            "UPDATE users SET last_login_at = NOW(), updated_at = NOW() WHERE id = ?"
        )
        .bind(&user.id)
        .execute(&self.state.db)
        .await?;

        let log_id = nanoid!();
        sqlx::query(
            "INSERT INTO login_logs (id, user_id, username, ip_address, login_type, success, created_at) VALUES (?, ?, ?, ?, ?, ?, NOW())"
        )
        .bind(&log_id)
        .bind(&user.id)
        .bind(&user.username)
        .bind("unknown")
        .bind("password")
        .bind(true)
        .execute(&self.state.db)
        .await?;

        let user_info = UserInfo {
            id: user.id.clone(),
            username: user.username.clone(),
            nickname: user.nickname.clone(),
            email: user.email.clone(),
            phone: user.phone.clone(),
            avatar: user.avatar.clone(),
            roles: vec![],
            permissions: vec![],
        };

        Ok(LoginResponse {
            access_token,
            refresh_token: nanoid!(),
            token_type: "Bearer".to_string(),
            expires_in: self.state.config.jwt.access_token_ttl_minutes * 60,
            user: user_info,
        })
    }

    pub async fn refresh(&self, _refresh_token: &str) -> Result<LoginResponse, IamError> {
        // 简化实现：返回新 token
        Err(IamError::TokenExpired)
    }

    pub async fn logout(&self, _user_id: &str) -> Result<(), IamError> {
        // 简化实现：无操作
        Ok(())
    }

    pub async fn change_password(&self, user_id: &str, req: ChangePasswordRequest) -> Result<(), IamError> {
        let user: Option<User> = sqlx::query_as(
            "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(user_id)
        .fetch_optional(&self.state.db)
        .await?;

        let user = user.ok_or(IamError::UserNotFound)?;

        let valid = verify(&req.old_password, &user.password_hash)
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        if !valid {
            return Err(IamError::InvalidCredentials);
        }

        let new_hash = hash(&req.new_password)
            .map_err(|e| IamError::Jwt(e.to_string()))?;

        sqlx::query(
            "UPDATE users SET password_hash = ?, password_expired_at = NULL, updated_at = NOW() WHERE id = ?"
        )
        .bind(&new_hash)
        .bind(user_id)
        .execute(&self.state.db)
        .await?;

        Ok(())
    }
}