use crate::{
    dto::{ChangePasswordRequest, LoginRequest, LoginResponse, UserInfo},
    error::IamError,
    models::RefreshToken,
    models::User,
    state::AppState,
};
use chrono::{Duration, Utc};
use digua_common_crypto::password::{hash, verify};
use digua_common_db::{db_execute, db_fetch_all, db_fetch_optional};
use digua_common_event::events::{IamLoginFailed, IamLoginSucceeded};
use digua_common_event::{AnyTx, Envelope, OutboxRepo};
use nanoid::nanoid;
use redis::AsyncCommands;
use sha2::{Digest, Sha256};

pub struct AuthService {
    state: AppState,
}

impl AuthService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn login(
        &self,
        req: LoginRequest,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<LoginResponse, IamError> {
        let tenant_slug = req.tenant_slug.as_deref().unwrap_or("default");
        // NOW()/`?` 占位符均方言通用（PG 的 NOW() 是 now() 的大小写不敏感别名）。
        let tenant: Option<(i64,)> = db_fetch_optional!(
            &self.state.db,
            (i64,),
            "SELECT id FROM iam_tenants WHERE slug = ? AND status = 'active'",
            tenant_slug
        )?;
        let tenant_id = match tenant {
            Some((id,)) => id,
            None => {
                self.record_login_failure(
                    &req.username,
                    None,
                    ip_address,
                    user_agent,
                    "tenant_not_found",
                )
                .await?;
                return Err(IamError::InvalidCredentials);
            }
        };

        let user: Option<User> = db_fetch_optional!(
            &self.state.db,
            User,
            "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users WHERE tenant_id = ? AND username = ? AND deleted_at IS NULL",
            tenant_id,
            &req.username
        )?;

        let user = match user {
            Some(u) => u,
            None => {
                self.record_login_failure(
                    &req.username,
                    None,
                    ip_address,
                    user_agent,
                    "user_not_found",
                )
                .await?;
                return Err(IamError::InvalidCredentials);
            }
        };

        if !user.status.is_active() {
            self.record_login_failure(
                &user.username,
                Some(&user.id),
                ip_address,
                user_agent,
                "user_disabled",
            )
            .await?;
            return Err(IamError::UserDisabled);
        }

        if let Some(expired_at) = user.password_expired_at {
            if expired_at < Utc::now() {
                self.record_login_failure(
                    &user.username,
                    Some(&user.id),
                    ip_address,
                    user_agent,
                    "password_expired",
                )
                .await?;
                return Err(IamError::PasswordExpired);
            }
        }

        let valid =
            verify(&req.password, &user.password_hash).map_err(|e| IamError::Jwt(e.to_string()))?;

        if !valid {
            self.record_login_failure(
                &user.username,
                Some(&user.id),
                ip_address,
                user_agent,
                "bad_password",
            )
            .await?;
            return Err(IamError::InvalidCredentials);
        }

        // 会话签发（access + refresh + user info）由 issue_session 统一负责，
        // 登录与邀请接受共用同一路径，避免两套 token 逻辑漂移。
        let response = self.issue_session(&user, ip_address, user_agent).await?;

        // 更新最后登录信息
        db_execute!(
            &self.state.db,
            "UPDATE iam_users SET last_login_at = NOW(), last_login_ip = ?, updated_at = NOW() WHERE id = ?",
            ip_address,
            &user.id
        )?;

        // 记录登录成功
        self.record_login_log(
            &user.id,
            &user.username,
            ip_address,
            user_agent,
            "password",
            true,
            None,
        )
        .await?;

        // 发布 iam.login.succeeded 事件（事务性 outbox）——事务双臂（prue 样板）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let mut envelope = Envelope::new(
                    "iam-service",
                    IamLoginSucceeded {
                        user_id: user.id.clone(),
                        ip: ip_address.to_string(),
                        user_agent: Some(user_agent.to_string()),
                        method: "password".to_string(),
                    },
                )
                .with_tenant_id(user.tenant_id.to_string());
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Mysql(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;
                tx.commit().await?;
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let mut envelope = Envelope::new(
                    "iam-service",
                    IamLoginSucceeded {
                        user_id: user.id.clone(),
                        ip: ip_address.to_string(),
                        user_agent: Some(user_agent.to_string()),
                        method: "password".to_string(),
                    },
                )
                .with_tenant_id(user.tenant_id.to_string());
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Postgres(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;
                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "login outbox: dialect feature not enabled".into(),
                ))
            }
        }

        Ok(response)
    }

    /// Issues an access token + refresh token + user info for an already
    /// authenticated user. Shared by password login and invitation acceptance.
    pub async fn issue_session(
        &self,
        user: &User,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<LoginResponse, IamError> {
        let roles = self.get_user_roles(&user.id).await?;
        let permissions = self.get_user_permissions(&user.id).await?;

        let cross_tenant = roles.iter().any(|r| r == "platform_admin");
        let ttl_secs = self.state.config.jwt.access_token_ttl_minutes * 60;
        let claims = serde_json::json!({
            "tid": user.tenant_id,
            "ct": cross_tenant,
            "username": user.username,
            "roles": roles,
            "permissions": permissions,
        });
        let access_token = self
            .state
            .token_forward
            .issue_user_jwt(&user.id, &user.username, claims, ttl_secs)
            .await
            .map_err(|e| IamError::Jwt(e.to_string()))?;

        let refresh_token = nanoid!(64);
        let refresh_token_hash = Self::hash_token(&refresh_token);
        let refresh_token_id = nanoid!();
        let refresh_expires_at = Utc::now() + Duration::days(30);

        db_execute!(
            &self.state.db,
            "INSERT INTO iam_refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, NOW(), NOW())",
            &refresh_token_id,
            &user.id,
            &refresh_token_hash,
            refresh_expires_at,
            ip_address,
            user_agent
        )?;

        Ok(LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.state.config.jwt.access_token_ttl_minutes * 60,
            user: UserInfo {
                id: user.id.clone(),
                username: user.username.clone(),
                nickname: user.nickname.clone(),
                email: user.email.clone(),
                phone: user.phone.clone(),
                avatar: user.avatar.clone(),
                roles,
                permissions,
            },
        })
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<LoginResponse, IamError> {
        let token_hash = Self::hash_token(refresh_token);

        let token: Option<RefreshToken> = db_fetch_optional!(
            &self.state.db,
            RefreshToken,
            "SELECT id, user_id, token_hash, expires_at, client_ip, user_agent, revoked, revoked_at, created_at, updated_at FROM iam_refresh_tokens WHERE token_hash = ?",
            &token_hash
        )?;

        let token = token.ok_or(IamError::TokenExpired)?;

        if token.revoked {
            return Err(IamError::TokenRevoked);
        }

        if token.expires_at < Utc::now() {
            return Err(IamError::TokenExpired);
        }

        // 获取用户信息
        let user: Option<User> = db_fetch_optional!(
            &self.state.db,
            User,
            "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users WHERE id = ? AND deleted_at IS NULL",
            &token.user_id
        )?;

        let user = user.ok_or(IamError::UserNotFound)?;

        if !user.status.is_active() {
            return Err(IamError::UserDisabled);
        }

        let roles = self.get_user_roles(&user.id).await?;
        let permissions = self.get_user_permissions(&user.id).await?;

        let cross_tenant = roles.iter().any(|r| r == "platform_admin");
        let ttl_secs = self.state.config.jwt.access_token_ttl_minutes * 60;
        let claims = serde_json::json!({
            "tid": user.tenant_id,
            "ct": cross_tenant,
            "username": user.username,
            "roles": roles,
            "permissions": permissions,
        });
        let access_token = self
            .state
            .token_forward
            .issue_user_jwt(&user.id, &user.username, claims, ttl_secs)
            .await
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

    pub async fn logout(&self, user_id: &str, jti: &str) -> Result<(), IamError> {
        // revoked/TRUE/FALSE 字面量两侧通用（MySQL 布尔即 tinyint 别名）。
        db_execute!(
            &self.state.db,
            "UPDATE iam_refresh_tokens SET revoked = TRUE, revoked_at = NOW() WHERE user_id = ? AND revoked = FALSE",
            user_id
        )?;

        if let Some(conn) = self.state.redis.as_ref() {
            if !jti.is_empty() {
                let mut conn = conn.clone();
                let key = format!("{}blacklist:{}", self.state.redis_prefix, jti);
                let ttl_sec = self.state.config.jwt.access_token_ttl_minutes * 60;
                if let Err(e) = conn.set_ex::<_, _, ()>(&key, "1", ttl_sec).await {
                    tracing::warn!(error = %e, jti, "iam logout blacklist write failed");
                }
            }
        }

        Ok(())
    }

    pub async fn revoke_session(&self, session_id: &str) -> Result<(), IamError> {
        db_execute!(
            &self.state.db,
            "UPDATE iam_refresh_tokens SET revoked = TRUE, revoked_at = NOW() WHERE id = ?",
            session_id
        )?;

        Ok(())
    }

    pub async fn get_user_sessions(&self, user_id: &str) -> Result<Vec<RefreshToken>, IamError> {
        let tokens: Vec<RefreshToken> = db_fetch_all!(
            &self.state.db,
            RefreshToken,
            "SELECT id, user_id, token_hash, expires_at, client_ip, user_agent, revoked, revoked_at, created_at, updated_at FROM iam_refresh_tokens WHERE user_id = ? AND revoked = FALSE ORDER BY created_at DESC",
            user_id
        )?;

        Ok(tokens)
    }

    pub async fn change_password(
        &self,
        user_id: &str,
        req: ChangePasswordRequest,
    ) -> Result<(), IamError> {
        let user: Option<User> = db_fetch_optional!(
            &self.state.db,
            User,
            "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users WHERE id = ? AND deleted_at IS NULL",
            user_id
        )?;

        let user = user.ok_or(IamError::UserNotFound)?;

        let valid = verify(&req.old_password, &user.password_hash)
            .map_err(|e| IamError::Jwt(e.to_string()))?;

        if !valid {
            return Err(IamError::InvalidCredentials);
        }

        let new_hash = hash(&req.new_password).map_err(|e| IamError::Jwt(e.to_string()))?;

        // 更新密码并将密码设置为 90 天后过期
        let password_expired_at = Utc::now() + Duration::days(90);
        db_execute!(
            &self.state.db,
            "UPDATE iam_users SET password_hash = ?, password_expired_at = ?, updated_at = NOW() WHERE id = ?",
            &new_hash,
            password_expired_at,
            user_id
        )?;

        Ok(())
    }

    async fn get_user_roles(&self, user_id: &str) -> Result<Vec<String>, IamError> {
        let roles: Vec<(String,)> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT r.code FROM iam_roles r INNER JOIN iam_user_roles ur ON r.id = ur.role_id WHERE ur.user_id = ? AND r.status = 'active' AND r.deleted_at IS NULL",
            user_id
        )?;

        Ok(roles.into_iter().map(|(code,)| code).collect())
    }

    async fn get_user_permissions(&self, user_id: &str) -> Result<Vec<String>, IamError> {
        let permissions: Vec<(String,)> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT DISTINCT p.code FROM iam_permissions p INNER JOIN iam_role_permissions rp ON p.id = rp.permission_id INNER JOIN iam_user_roles ur ON rp.role_id = ur.role_id WHERE ur.user_id = ? AND p.status = 'enabled' AND p.deleted_at IS NULL",
            user_id
        )?;

        Ok(permissions.into_iter().map(|(code,)| code).collect())
    }

    #[allow(clippy::too_many_arguments)] // Audit-log event: one row per login attempt; coalescing into a struct would obscure the call site.
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
        db_execute!(
            &self.state.db,
            "INSERT INTO iam_login_logs (id, user_id, username, ip_address, user_agent, login_type, success, error_message, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())",
            &log_id,
            user_id,
            username,
            ip_address,
            user_agent,
            login_type,
            success,
            error_message
        )?;

        Ok(())
    }

    async fn record_login_failure(
        &self,
        username: &str,
        user_id: Option<&str>,
        ip: &str,
        user_agent: &str,
        reason: &str,
    ) -> Result<(), IamError> {
        // 单 INSERT + outbox enqueue 的混合事务 → 双臂（prue 样板）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                sqlx::query(
                    "INSERT INTO iam_login_logs (id, user_id, username, ip_address, user_agent, \
                     login_type, success, error_message, created_at) \
                     VALUES (?, ?, ?, ?, ?, 'password', false, ?, NOW())",
                )
                .bind(nanoid!())
                .bind(user_id)
                .bind(username)
                .bind(ip)
                .bind(user_agent)
                .bind(reason)
                .execute(&mut *tx)
                .await?;
                let mut envelope = Envelope::new(
                    "iam-service",
                    IamLoginFailed {
                        username: username.to_string(),
                        user_id: user_id.map(|s| s.to_string()),
                        ip: ip.to_string(),
                        user_agent: Some(user_agent.to_string()),
                        reason: reason.to_string(),
                    },
                )
                .with_tenant_id("1");
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Mysql(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;
                tx.commit().await?;
            }
            // PG 臂：$N 占位符 + 小写 false（PG 布尔字面量大小写不敏感）。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                sqlx::query(
                    "INSERT INTO iam_login_logs (id, user_id, username, ip_address, user_agent, \
                     login_type, success, error_message, created_at) \
                     VALUES ($1, $2, $3, $4, $5, 'password', false, $6, NOW())",
                )
                .bind(nanoid!())
                .bind(user_id)
                .bind(username)
                .bind(ip)
                .bind(user_agent)
                .bind(reason)
                .execute(&mut *tx)
                .await?;
                let mut envelope = Envelope::new(
                    "iam-service",
                    IamLoginFailed {
                        username: username.to_string(),
                        user_id: user_id.map(|s| s.to_string()),
                        ip: ip.to_string(),
                        user_agent: Some(user_agent.to_string()),
                        reason: reason.to_string(),
                    },
                )
                .with_tenant_id("1");
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Postgres(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;
                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "record_login_failure: dialect feature not enabled".into(),
                ))
            }
        }
        Ok(())
    }

    fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }
}
