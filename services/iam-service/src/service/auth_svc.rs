use crate::{
    error::IamError,
    models::{LoginLog, RefreshToken, User},
    state::AppState,
    dto::{LoginRequest, LoginResponse, RefreshResponse, ChangePasswordRequest, UserInfo},
};
use common_crypto::password::{hash_password, verify_password};
use chrono::{DateTime, Utc};
use nanoid::nanoid;
use tracing::info;

pub struct AuthService {
    state: AppState,
}

impl AuthService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse, IamError> {
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE username = ? AND deleted_at IS NULL",
            req.username
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::InvalidCredentials)?;

        if user.status != UserStatus::Active {
            return Err(IamError::UserDisabled);
        }

        if let Some(expired_at) = user.password_expired_at {
            if expired_at < Utc::now() {
                return Err(IamError::PasswordExpired);
            }
        }

        if !verify_password(&req.password, &user.password_hash, &user.salt)? {
            self.log_login_failure(&req.username, "password mismatch").await?;
            return Err(IamError::InvalidCredentials);
        }

        let (access_token, refresh_token) = self.generate_tokens(&user).await?;

        self.update_last_login(&user.id).await?;
        self.log_login_success(&user).await?;

        let user_info = self.build_user_info(&user).await?;

        Ok(LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.state.config.jwt.access_token_ttl_minutes * 60,
            user: user_info,
        })
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<RefreshResponse, IamError> {
        let token_hash = common_crypto::hash_sha256(refresh_token);
        
        let db_token = sqlx::query_as!(
            RefreshToken,
            "SELECT * FROM refresh_tokens WHERE token_hash = ? AND revoked = false AND expires_at > ?",
            token_hash,
            Utc::now()
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::TokenExpired)?;

        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL",
            db_token.user_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::UserNotFound)?;

        self.revoke_token(&db_token.id).await?;
        
        let (access_token, new_refresh_token) = self.generate_tokens(&user).await?;

        Ok(RefreshResponse {
            access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.state.config.jwt.access_token_ttl_minutes * 60,
        })
    }

    pub async fn logout(&self, user_id: &str, refresh_token: Option<&str>) -> Result<(), IamError> {
        if let Some(token) = refresh_token {
            let token_hash = common_crypto::hash_sha256(token);
            sqlx::query!(
                "UPDATE refresh_tokens SET revoked = true, revoked_at = ? WHERE token_hash = ?",
                Utc::now(),
                token_hash
            )
            .execute(&self.state.db)
            .await?;
        }

        sqlx::query!(
            "UPDATE refresh_tokens SET revoked = true, revoked_at = ? WHERE user_id = ?",
            Utc::now(),
            user_id
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    pub async fn change_password(&self, user_id: &str, req: ChangePasswordRequest) -> Result<(), IamError> {
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL",
            user_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::UserNotFound)?;

        if !verify_password(&req.old_password, &user.password_hash, &user.salt)? {
            return Err(IamError::InvalidCredentials);
        }

        let (new_hash, new_salt) = hash_password(&req.new_password)?;
        
        sqlx::query!(
            "UPDATE users SET password_hash = ?, salt = ?, password_expired_at = NULL, updated_at = ? WHERE id = ?",
            new_hash,
            new_salt,
            Utc::now(),
            user_id
        )
        .execute(&self.state.db)
        .await?;

        self.logout(user_id, None).await?;

        Ok(())
    }

    async fn generate_tokens(&self, user: &User) -> Result<(String, String), IamError> {
        let token_service = TokenService::new(self.state.clone())?;
        let access_token = token_service.issue_access_token(&user.id, &user.username, vec![], vec![])?;
        
        let refresh_token = nanoid!();
        let token_hash = common_crypto::hash_sha256(&refresh_token);
        let expires_at = Utc::now() + chrono::Duration::days(self.state.config.jwt.refresh_token_ttl_days as i64);

        sqlx::query!(
            "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, revoked) VALUES (?, ?, ?, ?, ?, ?, ?)",
            nanoid!(),
            user.id,
            token_hash,
            expires_at,
            "unknown",
            None::<String>,
            false
        )
        .execute(&self.state.db)
        .await?;

        Ok((access_token, refresh_token))
    }

    async fn revoke_token(&self, token_id: &str) -> Result<(), IamError> {
        sqlx::query!(
            "UPDATE refresh_tokens SET revoked = true, revoked_at = ? WHERE id = ?",
            Utc::now(),
            token_id
        )
        .execute(&self.state.db)
        .await?;
        Ok(())
    }

    async fn update_last_login(&self, user_id: &str) -> Result<(), IamError> {
        sqlx::query!(
            "UPDATE users SET last_login_at = ?, updated_at = ? WHERE id = ?",
            Utc::now(),
            Utc::now(),
            user_id
        )
        .execute(&self.state.db)
        .await?;
        Ok(())
    }

    async fn log_login_success(&self, user: &User) -> Result<(), IamError> {
        let log = LoginLog {
            id: nanoid!(),
            user_id: Some(user.id.clone()),
            username: user.username.clone(),
            ip_address: "unknown".to_string(),
            user_agent: None,
            location: None,
            login_type: LoginType::Password,
            success: true,
            error_message: None,
            created_at: Utc::now(),
        };

        sqlx::query!(
            "INSERT INTO login_logs (id, user_id, username, ip_address, login_type, success, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            log.id,
            log.user_id,
            log.username,
            log.ip_address,
            "password",
            log.success,
            log.created_at
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    async fn log_login_failure(&self, username: &str, error: &str) -> Result<(), IamError> {
        let log = LoginLog {
            id: nanoid!(),
            user_id: None,
            username: username.to_string(),
            ip_address: "unknown".to_string(),
            user_agent: None,
            location: None,
            login_type: LoginType::Password,
            success: false,
            error_message: Some(error.to_string()),
            created_at: Utc::now(),
        };

        sqlx::query!(
            "INSERT INTO login_logs (id, username, ip_address, login_type, success, error_message, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            log.id,
            log.username,
            log.ip_address,
            "password",
            log.success,
            log.error_message,
            log.created_at
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    async fn build_user_info(&self, user: &User) -> Result<UserInfo, IamError> {
        Ok(UserInfo {
            id: user.id.clone(),
            username: user.username.clone(),
            nickname: user.nickname.clone(),
            email: user.email.clone(),
            phone: user.phone.clone(),
            avatar: user.avatar.clone(),
            roles: vec![],
            permissions: vec![],
        })
    }
}
