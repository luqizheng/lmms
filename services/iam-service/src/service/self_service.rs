//! Self-service flows: password reset, invitation acceptance and self profile.
//!
//! Tokens are random 64-char strings. Only their SHA-256 hash is persisted, and
//! consumption is a conditional UPDATE so concurrent replays cannot win twice.

use crate::dto::AcceptInvitationRequest;
use crate::dto::LoginResponse;
use crate::error::IamError;
use crate::mail::MailError;
use crate::models::User;
use crate::service::AuthService;
use crate::state::AppState;
use chrono::{Duration, Utc};
use digua_common_crypto::password::hash;
use digua_common_db::{db_execute, db_fetch_optional};
use nanoid::nanoid;
use sha2::{Digest, Sha256};

pub const KIND_PASSWORD_RESET: &str = "password_reset";
pub const KIND_INVITATION: &str = "invitation";

const RESET_TTL_MINUTES: i64 = 30;
const INVITATION_TTL_DAYS: i64 = 7;

const USER_COLUMNS: &str = "id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at";

pub struct SelfService {
    state: AppState,
}

impl SelfService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Issues a reset token for an active account and mails the link.
    /// Unknown or disabled accounts return `Ok(())` to prevent enumeration.
    pub async fn request_password_reset(&self, email: &str) -> Result<(), IamError> {
        let user: Option<User> = db_fetch_optional!(
            &self.state.db,
            User,
            &format!(
                "SELECT {USER_COLUMNS} FROM iam_users WHERE email = ? AND status = 'active' AND deleted_at IS NULL LIMIT 1"
            ),
            email
        )?;

        let Some(user) = user else {
            return Ok(());
        };

        self.issue_token_and_mail(&user, KIND_PASSWORD_RESET, Duration::minutes(RESET_TTL_MINUTES))
            .await
    }

    /// Consumes a reset token and replaces the password, revoking all sessions.
    pub async fn reset_password(&self, token: &str, new_password: &str) -> Result<(), IamError> {
        let token_hash = hash_token(token);
        let row: Option<(String, String)> = db_fetch_optional!(
            &self.state.db,
            (String, String),
            "SELECT id, user_id FROM iam_one_time_tokens \
             WHERE token_hash = ? AND kind = ? AND consumed_at IS NULL AND expires_at > NOW()",
            &token_hash,
            KIND_PASSWORD_RESET
        )?;
        let (id, user_id) = row.ok_or(IamError::InvalidOrExpiredToken)?;

        self.consume(&id).await?;

        let new_hash = hash(new_password).map_err(|e| IamError::Internal(e.to_string()))?;
        db_execute!(
            &self.state.db,
            "UPDATE iam_users SET password_hash = ?, password_expired_at = NULL, updated_at = NOW() WHERE id = ?",
            &new_hash,
            &user_id
        )?;
        self.revoke_all_sessions(&user_id).await?;

        Ok(())
    }

    /// Issues an invitation token for a freshly created account.
    pub async fn invite_user(&self, user_id: &str) -> Result<(), IamError> {
        let user: Option<User> = db_fetch_optional!(
            &self.state.db,
            User,
            &format!("SELECT {USER_COLUMNS} FROM iam_users WHERE id = ? AND deleted_at IS NULL"),
            user_id
        )?;
        let user = user.ok_or(IamError::UserNotFound)?;
        if user.email.as_deref().unwrap_or("").trim().is_empty() {
            return Err(IamError::EmailRequired);
        }
        self.issue_token_and_mail(&user, KIND_INVITATION, Duration::days(INVITATION_TTL_DAYS))
            .await
    }

    /// Consumes an invitation token, activates the account and returns a session.
    pub async fn accept_invitation(
        &self,
        req: AcceptInvitationRequest,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<LoginResponse, IamError> {
        let token_hash = hash_token(&req.token);
        let row: Option<(String, String)> = db_fetch_optional!(
            &self.state.db,
            (String, String),
            "SELECT id, user_id FROM iam_one_time_tokens \
             WHERE token_hash = ? AND kind = ? AND consumed_at IS NULL AND expires_at > NOW()",
            &token_hash,
            KIND_INVITATION
        )?;
        let (id, user_id) = row.ok_or(IamError::InvalidOrExpiredToken)?;

        self.consume(&id).await?;

        let new_hash = hash(&req.new_password).map_err(|e| IamError::Internal(e.to_string()))?;
        db_execute!(
            &self.state.db,
            "UPDATE iam_users SET password_hash = ?, nickname = COALESCE(?, nickname), \
             email_verified = TRUE, updated_at = NOW() WHERE id = ?",
            &new_hash,
            &req.nickname,
            &user_id
        )?;

        let user: Option<User> = db_fetch_optional!(
            &self.state.db,
            User,
            &format!("SELECT {USER_COLUMNS} FROM iam_users WHERE id = ? AND deleted_at IS NULL"),
            &user_id
        )?;
        let user = user.ok_or(IamError::UserNotFound)?;

        AuthService::new(self.state.clone())
            .issue_session(&user, ip_address, user_agent)
            .await
    }

    /// Persists a hashed token, retires the previous unconsumed one and mails
    /// the link. A failed delivery invalidates the token instead of leaving a
    /// valid-but-undeliverable secret in the database.
    async fn issue_token_and_mail(
        &self,
        user: &User,
        kind: &str,
        ttl: Duration,
    ) -> Result<(), IamError> {
        if !self.state.mailer.is_configured() {
            return Err(IamError::MailNotConfigured);
        }
        let email = user.email.clone().ok_or(IamError::EmailRequired)?;

        db_execute!(
            &self.state.db,
            "UPDATE iam_one_time_tokens SET consumed_at = NOW() \
             WHERE user_id = ? AND kind = ? AND consumed_at IS NULL",
            &user.id,
            kind
        )?;

        let token = nanoid!(64);
        let token_hash = hash_token(&token);
        let id = nanoid!();
        let expires_at = Utc::now() + ttl;

        db_execute!(
            &self.state.db,
            "INSERT INTO iam_one_time_tokens \
             (id, tenant_id, user_id, kind, token_hash, expires_at, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, NOW())",
            &id,
            user.tenant_id,
            &user.id,
            kind,
            &token_hash,
            expires_at
        )?;

        let (subject, html) = self.build_mail(user, kind, &token)?;
        if let Err(e) = self.state.mailer.send_html(&email, &subject, html).await {
            let _ = db_execute!(
                &self.state.db,
                "UPDATE iam_one_time_tokens SET consumed_at = NOW() WHERE id = ?",
                &id
            );
            return Err(match e {
                MailError::NotConfigured => IamError::MailNotConfigured,
                MailError::Delivery(msg) => IamError::MailDeliveryFailed(msg),
            });
        }

        Ok(())
    }

    async fn consume(&self, token_id: &str) -> Result<(), IamError> {
        let affected = db_execute!(
            &self.state.db,
            "UPDATE iam_one_time_tokens SET consumed_at = NOW() WHERE id = ? AND consumed_at IS NULL",
            token_id
        )?;
        if affected == 0 {
            return Err(IamError::InvalidOrExpiredToken);
        }
        Ok(())
    }

    async fn revoke_all_sessions(&self, user_id: &str) -> Result<(), IamError> {
        db_execute!(
            &self.state.db,
            "UPDATE iam_refresh_tokens SET revoked = TRUE, revoked_at = NOW() WHERE user_id = ? AND revoked = FALSE",
            user_id
        )?;
        Ok(())
    }

    fn build_mail(&self, user: &User, kind: &str, token: &str) -> Result<(String, String), IamError> {
        let base = self.state.config.public.base_url.trim_end_matches('/');
        let name = if user.nickname.is_empty() {
            user.username.clone()
        } else {
            user.nickname.clone()
        };

        match kind {
            KIND_PASSWORD_RESET => Ok((
                "重置你的 AI R&D Workspace 密码".to_string(),
                format!(
                    "<p>{name}，你好：</p>\
                     <p>我们收到了重置密码的请求。点击下面的链接设置新密码，链接 30 分钟内有效：</p>\
                     <p><a href=\"{base}/auth/reset-password?token={token}\">重置密码</a></p>\
                     <p>如果这不是你的操作，请忽略本邮件。</p>"
                ),
            )),
            KIND_INVITATION => Ok((
                "邀请你加入 AI R&D Workspace".to_string(),
                format!(
                    "<p>{name}，你好：</p>\
                     <p>你的账号已经创建。点击下面的链接设置密码并加入工作区，链接 7 天内有效：</p>\
                     <p><a href=\"{base}/auth/invitation?token={token}\">接受邀请</a></p>"
                ),
            )),
            _ => Err(IamError::Internal(format!("unknown token kind: {kind}"))),
        }
    }
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}