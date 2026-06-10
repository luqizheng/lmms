use crate::config::LoginSecurityCfg;
use crate::db::DbPool;
use crate::error::AppError;
use crate::models::login_log::{FAIL_BAD_PASSWORD, FAIL_LOCKED, FAIL_USER_NOT_FOUND};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use nanoid::nanoid;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use tracing::warn;

const KEY_FAIL: &str = "auth:login:fail";
const KEY_LOCK: &str = "auth:login:lock";
const KEY_CODE: &str = "auth:code";

#[derive(Debug)]
pub enum LoginOutcome {
    Success { user_id: u64, code: String },
    Locked { retry_after_secs: u64 },
    Invalid,
}

pub struct LoginCtx<'a> {
    pub pool: &'a DbPool,
    pub redis: &'a redis::Client,
    pub cfg: &'a LoginSecurityCfg,
    pub ip: String,
    pub user_agent: String,
}

fn dummy_hash() -> String {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(b"dummy-do-not-match", &salt)
                .expect("hash dummy")
                .to_string()
        })
        .clone()
}

pub async fn login(
    ctx: &LoginCtx<'_>,
    username: &str,
    password: &str,
) -> Result<LoginOutcome, AppError> {
    if let Some(retry) = check_lock(ctx, username).await? {
        write_log(ctx, None, username, FAIL_LOCKED, false).await;
        return Ok(LoginOutcome::Locked {
            retry_after_secs: retry,
        });
    }

    let row = fetch_user_for_login(ctx.pool, username).await?;
    let verify_ok = match &row {
        Some((_uid, hash)) => verify_password(password, hash),
        None => verify_password(password, &dummy_hash()),
    };

    let user_id = row.as_ref().map(|(id, _)| *id);

    match (user_id, verify_ok) {
        (Some(uid), true) => {
            clear_failure(ctx, username).await?;
            update_last_login(ctx, uid).await?;
            let code = issue_code(ctx, uid).await?;
            write_log(ctx, Some(uid), username, "", true).await;
            Ok(LoginOutcome::Success { user_id: uid, code })
        }
        _ => {
            let reason = if user_id.is_some() {
                FAIL_BAD_PASSWORD
            } else {
                FAIL_USER_NOT_FOUND
            };
            incr_failure(ctx, username).await?;
            write_log(ctx, user_id, username, reason, false).await;
            Ok(LoginOutcome::Invalid)
        }
    }
}

async fn check_lock(ctx: &LoginCtx<'_>, username: &str) -> Result<Option<u64>, AppError> {
    let mut conn = ctx
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let key = format!("{KEY_LOCK}:{username}");
    let ttl: i64 = conn.ttl(&key).await.unwrap_or(-2);
    if ttl > 0 {
        return Ok(Some(ttl as u64));
    }
    Ok(None)
}

async fn fetch_user_for_login(
    pool: &DbPool,
    username: &str,
) -> Result<Option<(u64, String)>, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let row: Option<(u64, Option<String>)> = sqlx::query_as(
                "SELECT id, password_hash FROM users WHERE username = ? AND status = 1",
            )
            .bind(username)
            .fetch_optional(p)
            .await?;
            Ok(row.and_then(|(id, h)| h.map(|h| (id, h))))
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

fn verify_password(password: &str, hash_str: &str) -> bool {
    let parsed = match PasswordHash::new(hash_str) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

async fn incr_failure(ctx: &LoginCtx<'_>, username: &str) -> Result<(), AppError> {
    let mut conn = ctx
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let key = format!("{KEY_FAIL}:{username}");
    let n: i64 = conn
        .incr(&key, 1)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if n == 1 {
        let _: () = conn
            .expire(&key, ctx.cfg.counter_window.as_secs() as i64)
            .await
            .unwrap_or(());
    }
    if n >= ctx.cfg.max_failures as i64 {
        let lock_key = format!("{KEY_LOCK}:{username}");
        let _: () = conn
            .set_ex(&lock_key, "1", ctx.cfg.lockout_duration.as_secs() as i64)
            .await
            .unwrap_or(());
    }
    Ok(())
}

async fn clear_failure(ctx: &LoginCtx<'_>, username: &str) -> Result<(), AppError> {
    let mut conn = ctx
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let _: () = conn
        .del::<_, ()>(format!("{KEY_FAIL}:{username}"))
        .await
        .unwrap_or(());
    let _: () = conn
        .del::<_, ()>(format!("{KEY_LOCK}:{username}"))
        .await
        .unwrap_or(());
    Ok(())
}

async fn update_last_login(ctx: &LoginCtx<'_>, user_id: u64) -> Result<(), AppError> {
    if let DbPool::Mysql(p) = ctx.pool {
        sqlx::query("UPDATE users SET last_login_at = NOW(3), last_login_ip = ? WHERE id = ?")
            .bind(&ctx.ip)
            .bind(user_id)
            .execute(p)
            .await?;
    }
    Ok(())
}

async fn issue_code(ctx: &LoginCtx<'_>, user_id: u64) -> Result<String, AppError> {
    let code = nanoid!(32);
    let mut conn = ctx
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let key = format!("{KEY_CODE}:{code}");
    let _: () = conn
        .set_ex(&key, user_id as i64, ctx.cfg.code_ttl.as_secs() as i64)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(code)
}

async fn write_log(
    ctx: &LoginCtx<'_>,
    user_id: Option<u64>,
    username: &str,
    fail_reason: &str,
    success: bool,
) {
    if let DbPool::Mysql(p) = ctx.pool {
        let res = sqlx::query(
            "INSERT INTO user_login_log \
             (user_id, username, ip, user_agent, success, fail_reason, login_at) \
             VALUES (?, ?, ?, ?, ?, ?, NOW(3))",
        )
        .bind(user_id)
        .bind(username)
        .bind(&ctx.ip)
        .bind(truncate_ua(&ctx.user_agent))
        .bind(if success { 1i8 } else { 0i8 })
        .bind(if fail_reason.is_empty() {
            None
        } else {
            Some(fail_reason)
        })
        .execute(p)
        .await;
        if let Err(e) = res {
            warn!(error = %e, username = username, success, "write user_login_log failed");
        }
    }
}

fn truncate_ua(s: &str) -> String {
    if s.len() <= 255 {
        s.to_string()
    } else {
        s[..255].to_string()
    }
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(e.into()))
}

#[derive(Debug)]
pub enum RegisterError {
    Closed,
    UsernameTaken,
    InvalidInput(String),
    Internal(String),
}

pub async fn setup_status(pool: &DbPool) -> Result<bool, AppError> {
    if let DbPool::Mysql(p) = pool {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM users WHERE status = 1")
            .fetch_one(p)
            .await?;
        let c: i64 = row.try_get("c").unwrap_or(0);
        Ok(c == 0)
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn register_admin(
    pool: &DbPool,
    username: &str,
    password: &str,
    email: Option<String>,
    phone: Option<String>,
    nickname: Option<String>,
) -> Result<u64, RegisterError> {
    if !is_valid_username(username) {
        return Err(RegisterError::InvalidInput(
            "username must be 3-32 chars [a-zA-Z0-9_.-]".into(),
        ));
    }
    if password.len() < 6 {
        return Err(RegisterError::InvalidInput(
            "password must be at least 6 chars".into(),
        ));
    }
    if let Some(e) = email.as_deref() {
        if !e.is_empty() && !e.contains('@') {
            return Err(RegisterError::InvalidInput("invalid email".into()));
        }
    }
    if let Some(p) = phone.as_deref() {
        if !p.is_empty() && (p.len() != 11 || !p.chars().all(|c| c.is_ascii_digit())) {
            return Err(RegisterError::InvalidInput("invalid phone".into()));
        }
    }

    let hash = hash_password(password).map_err(|e| RegisterError::Internal(e.to_string()))?;

    if let DbPool::Mysql(p) = pool {
        let mut tx = p
            .begin()
            .await
            .map_err(|e| RegisterError::Internal(e.to_string()))?;

        let row = sqlx::query("SELECT COUNT(*) AS c FROM users WHERE status = 1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| RegisterError::Internal(e.to_string()))?;
        let c: i64 = row.try_get("c").unwrap_or(0);
        if c > 0 {
            return Err(RegisterError::Closed);
        }

        let existing: Option<(u64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| RegisterError::Internal(e.to_string()))?;
        if existing.is_some() {
            return Err(RegisterError::UsernameTaken);
        }

        let user_code = format!("U-{}", nanoid!(10));

        let res = sqlx::query(
            "INSERT INTO users (username, user_code, password_hash, email, phone, nickname, status) \
             VALUES (?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(username)
        .bind(&user_code)
        .bind(&hash)
        .bind(email.as_deref().unwrap_or(""))
        .bind(phone.as_deref().unwrap_or(""))
        .bind(nickname.as_deref().unwrap_or(username))
        .execute(&mut *tx)
        .await
        .map_err(|e| RegisterError::Internal(e.to_string()))?;
        let uid = res.last_insert_id();

        let role_row: Option<(u64,)> =
            sqlx::query_as("SELECT id FROM roles WHERE code = 'admin'")
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| RegisterError::Internal(e.to_string()))?;
        let role_id = role_row
            .ok_or_else(|| RegisterError::Internal("admin role not seeded".into()))?
            .0;

        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
            .bind(uid)
            .bind(role_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| RegisterError::Internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| RegisterError::Internal(e.to_string()))?;
        Ok(uid)
    } else {
        Err(RegisterError::Internal("unsupported db".into()))
    }
}

fn is_valid_username(u: &str) -> bool {
    let len = u.chars().count();
    if !(3..=32).contains(&len) {
        return false;
    }
    u.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_hash_parses_and_rejects_other_passwords() {
        let h = dummy_hash();
        assert!(PasswordHash::new(&h).is_ok());
        assert!(Argon2::default()
            .verify_password(
                b"definitely-not-the-dummy-password",
                &PasswordHash::new(&h).unwrap()
            )
            .is_err());
    }

    #[test]
    fn truncate_ua_bounds() {
        let long = "a".repeat(300);
        assert_eq!(truncate_ua(&long).len(), 255);
        assert_eq!(truncate_ua("short"), "short");
    }

    #[test]
    fn valid_username_ok() {
        assert!(is_valid_username("admin"));
        assert!(is_valid_username("user.name_1-2"));
        assert!(!is_valid_username("ab"));
        assert!(!is_valid_username("a".repeat(33).as_str()));
        assert!(!is_valid_username("用户"));
        assert!(!is_valid_username("a b"));
    }
}
