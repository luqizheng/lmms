use crate::{
    error::IamError,
    models::user::UserStatus,
    models::User,
    models::RefreshToken,
    state::AppState,
    dto::{LoginRequest, LoginResponse, ChangePasswordRequest, UserInfo},
    service::TokenService,
};
use common_crypto::password::{hash, verify};
use chrono::{Utc, Duration};
use nanoid::nanoid;
use sha2::{Sha256, Digest};

pub struct AuthService {
    state: AppState,
}

impl AuthService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn login(&self, req: LoginRequest, ip_address: &str, user_agent: &str) -> Result<LoginResponse, IamError> {
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
            // 记录登录失败
            self.record_login_log(&user.id, &user.username, ip_address, user_agent, "password", false, Some("invalid password"))
                .await?;
            return Err(IamError::InvalidCredentials);
        }

        let token_service = TokenService::new(self.state.clone())
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        // 获取用户角色和权限
        let roles = self.get_user_roles(&user.id).await?;
        let permissions = self.get_user_permissions(&user.id).await?;
        
        let access_token = token_service.issue_access_token(&user.id, &user.username, roles.clone(), permissions.clone())
            .map_err(|e| IamError::Jwt(e.to_string()))?;

        // 生成 refresh token
        let refresh_token = nanoid!(64);
        let refresh_token_hash = Self::hash_token(&refresh_token);
        let refresh_token_id = nanoid!();
        let refresh_expires_at = Utc::now() + Duration::days(30);

        sqlx::query(
            "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, NOW(), NOW())"
        )
        .bind(&refresh_token_id)
        .bind(&user.id)
        .bind(&refresh_token_hash)
        .bind(refresh_expires_at)
        .bind(ip_address)
        .bind(user_agent)
        .execute(&self.state.db)
        .await?;

        // 更新最后登录信息
        sqlx::query(
            "UPDATE users SET last_login_at = NOW(), last_login_ip = ?, updated_at = NOW() WHERE id = ?"
        )
        .bind(ip_address)
        .bind(&user.id)
        .execute(&self.state.db)
        .await?;

        // 记录登录成功
        self.record_login_log(&user.id, &user.username, ip_address, user_agent, "password", true, None)
            .await?;

        let user_info = UserInfo {
            id: user.id.clone(),
            username: user.username.clone(),
            nickname: user.nickname.clone(),
            email: user.email.clone(),
            phone: user.phone.clone(),
            avatar: user.avatar.clone(),
            roles: roles.clone(),
            permissions,
        };

        Ok(LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.state.config.jwt.access_token_ttl_minutes * 60,
            user: user_info,
        })
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<LoginResponse, IamError> {
        let token_hash = Self::hash_token(refresh_token);
        
        let token: Option<RefreshToken> = sqlx::query_as(
            "SELECT id, user_id, token_hash, expires_at, revoked, created_at FROM refresh_tokens WHERE token_hash = ?"
        )
        .bind(&token_hash)
        .fetch_optional(&self.state.db)
        .await?;

        let token = token.ok_or(IamError::TokenExpired)?;

        if token.revoked {
            return Err(IamError::TokenRevoked);
        }

        if token.expires_at < Utc::now() {
            return Err(IamError::TokenExpired);
        }

        // 获取用户信息
        let user: Option<User> = sqlx::query_as(
            "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&token.user_id)
        .fetch_optional(&self.state.db)
        .await?;

        let user = user.ok_or(IamError::UserNotFound)?;

        if user.status != UserStatus::Active {
            return Err(IamError::UserDisabled);
        }

        let token_service = TokenService::new(self.state.clone())
            .map_err(|e| IamError::Jwt(e.to_string()))?;
        
        let roles = self.get_user_roles(&user.id).await?;
        let permissions = self.get_user_permissions(&user.id).await?;
        
        let access_token = token_service.issue_access_token(&user.id, &user.username, roles.clone(), permissions.clone())
            .map_err(|e| IamError::Jwt(e.to_string()))?;

        let user_info = UserInfo {
            id: user.id.clone(),
            username: user.username.clone(),
            nickname: user.nickname.clone(),
            email: user.email.clone(),
            phone: user.phone.clone(),
            avatar: user.avatar.clone(),
            roles,
            permissions,
        };

        Ok(LoginResponse {
            access_token,
            refresh_token: refresh_token.to_string(),
            token_type: "Bearer".to_string(),
            expires_in: self.state.config.jwt.access_token_ttl_minutes * 60,
            user: user_info,
        })
    }

    pub async fn logout(&self, user_id: &str) -> Result<(), IamError> {
        // 撤销用户的所有 refresh tokens
        sqlx::query(
            "UPDATE refresh_tokens SET revoked = TRUE, revoked_at = NOW() WHERE user_id = ? AND revoked = FALSE"
        )
        .bind(user_id)
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    pub async fn revoke_session(&self, session_id: &str) -> Result<(), IamError> {
        sqlx::query(
            "UPDATE refresh_tokens SET revoked = TRUE, revoked_at = NOW() WHERE id = ?"
        )
        .bind(session_id)
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    pub async fn get_user_sessions(&self, user_id: &str) -> Result<Vec<RefreshToken>, IamError> {
        let tokens: Vec<RefreshToken> = sqlx::query_as(
            "SELECT id, user_id, token_hash, expires_at, client_ip, user_agent, revoked, revoked_at, created_at, updated_at FROM refresh_tokens WHERE user_id = ? AND revoked = FALSE ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.state.db)
        .await?;

        Ok(tokens)
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

        // 更新密码并将密码设置为 90 天后过期
        let password_expired_at = Utc::now() + Duration::days(90);
        sqlx::query(
            "UPDATE users SET password_hash = ?, password_expired_at = ?, updated_at = NOW() WHERE id = ?"
        )
        .bind(&new_hash)
        .bind(password_expired_at)
        .bind(user_id)
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    async fn get_user_roles(&self, user_id: &str) -> Result<Vec<String>, IamError> {
        let roles: Vec<(String,)> = sqlx::query_as(
            "SELECT r.code FROM roles r INNER JOIN user_roles ur ON r.id = ur.role_id WHERE ur.user_id = ? AND r.status = 'active' AND r.deleted_at IS NULL"
        )
        .bind(user_id)
        .fetch_all(&self.state.db)
        .await?;

        Ok(roles.into_iter().map(|(code,)| code).collect())
    }

    async fn get_user_permissions(&self, user_id: &str) -> Result<Vec<String>, IamError> {
        let permissions: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT p.code FROM permissions p INNER JOIN role_permissions rp ON p.id = rp.permission_id INNER JOIN user_roles ur ON rp.role_id = ur.role_id WHERE ur.user_id = ? AND p.status = 'enabled' AND p.deleted_at IS NULL"
        )
        .bind(user_id)
        .fetch_all(&self.state.db)
        .await?;

        Ok(permissions.into_iter().map(|(code,)| code).collect())
    }

    async fn record_login_log(
        &self,
        user_id: &str,
        username: &str,
        ip_address: &str,
        user_agent: &str,
        login_type: &str,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<(), IamError> {
        let log_id = nanoid!();
        sqlx::query(
            "INSERT INTO login_logs (id, user_id, username, ip_address, user_agent, login_type, success, error_message, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())"
        )
        .bind(&log_id)
        .bind(user_id)
        .bind(username)
        .bind(ip_address)
        .bind(user_agent)
        .bind(login_type)
        .bind(success)
        .bind(error_message)
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }
}
