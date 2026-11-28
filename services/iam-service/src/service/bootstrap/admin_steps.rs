//! Bootstrap IAM 本地步骤（S01~S04）：默认租户校验 + 首管理员及其双角色绑定。
//!
//! 每个 Step 遵循设计 Q6 统一语义：
//! Check（对象是否已存在）→ Insert（不存在才创建）→ Verify（核对关键
//! 属性）→ 报告结果。对象已存在且属性一致 → 视为幂等恢复成功；存在但
//! 属性不一致 → 报告冲突，**不得覆盖**。数据库唯一约束是防重复创建的
//! 最终保障（不依赖应用层 SELECT）。
//!
//! IAM 本地步骤各自使用本地事务；不假设与 Token Service 共享事务。

use digua_common_db::{db_fetch_one, db_fetch_optional, DbPool};
use digua_common_event::events::IamUserCreated;
use digua_common_event::AnyTx;
use digua_common_event::{Envelope, OutboxRepo};

use crate::dto::BootstrapRequest;
use crate::error::IamError;
use crate::state::AppState;

/// 默认租户 id（V003 迁移预置；platform_admin / tenant_admin seed 角色
/// 都挂在 tenant 1，首管理员必须落在同一租户，符合既有数据模型）。
pub const DEFAULT_TENANT_ID: i64 = 1;
/// V005 预置的平台管理员角色 id。
pub const PLATFORM_ADMIN_ROLE_ID: &str = "role-platform-admin-seed";

/// Step 执行失败分类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepFailure {
    /// 对象已存在但关键属性不一致——报告冲突，不得覆盖。
    Conflict(String),
    /// 执行失败（稳定 slug 摘要，不含凭据 / 拓扑细节；等待人工恢复）。
    Failed(&'static str),
    /// 跨服务依赖不可用（可稍后恢复）。
    Dependency(&'static str),
}

/// 首管理员凭据（来自当前 HTTP 请求，绝不持久化明文）。
#[derive(Debug, Clone)]
pub struct AdminCredentials {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub nickname: Option<String>,
}

impl AdminCredentials {
    /// 从请求体构造（仅当首管理员 Step 未完成时才要求完整凭据）。
    pub fn from_request(req: &BootstrapRequest) -> Result<Option<Self>, IamError> {
        let Some(username) = req.username.clone() else {
            return Err(IamError::BootstrapInvalidRequest);
        };
        let Some(password) = req.password.clone() else {
            return Err(IamError::BootstrapInvalidRequest);
        };
        let creds = Self {
            username,
            password,
            email: req.email.clone(),
            nickname: req.nickname.clone(),
        };
        creds
            .validate_fields()
            .map_err(|_| IamError::BootstrapInvalidRequest)?;
        Ok(Some(creds))
    }

    fn validate_fields(&self) -> Result<(), ()> {
        // username：字母数字下划线，3..=50。
        let u_ok = self.username.len() >= 3
            && self.username.len() <= 50
            && self
                .username
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !u_ok {
            return Err(());
        }
        if self.password.len() < 8 || self.password.len() > 128 {
            return Err(());
        }
        if let Some(email) = &self.email {
            if email.len() > 255 || !email.contains('@') || email.contains(' ') {
                return Err(());
            }
        }
        if let Some(nickname) = &self.nickname {
            if nickname.is_empty() || nickname.len() > 100 {
                return Err(());
            }
        }
        Ok(())
    }
}

/// sqlx 错误 → 唯一约束冲突判定（MySQL 1062 / PG 23505）。
pub(crate) fn is_duplicate_key(e: &sqlx::Error) -> bool {
    match e {
        sqlx::Error::Database(db) => is_duplicate_code(db.code().as_deref(), db.message()),
        _ => false,
    }
}

fn is_duplicate_code(code: Option<&str>, message: &str) -> bool {
    matches!(code, Some("1062") | Some("23505"))
        || message.contains("Duplicate entry")
        || message.contains("unique constraint")
        || message.contains("duplicate key")
}

// ── S01：默认租户存在性校验（Check-only）─────────────────────────────────

pub async fn run_default_tenant(pool: &DbPool) -> Result<(), StepFailure> {
    let row: Option<(String,)> = db_fetch_optional!(
        pool,
        (String,),
        "SELECT status FROM iam_tenants WHERE id = ?",
        DEFAULT_TENANT_ID
    )
    .map_err(|_| StepFailure::Failed("tenant_check_db_error"))?;
    match row {
        Some((status,)) if status == "active" => Ok(()),
        Some((status,)) => Err(StepFailure::Conflict(format!(
            "default tenant status is {status}"
        ))),
        None => Err(StepFailure::Failed("default tenant missing")),
    }
}

// ── S02：首管理员用户创建 ────────────────────────────────────────────────

pub async fn run_first_admin_user(
    state: &AppState,
    creds: &AdminCredentials,
) -> Result<(), StepFailure> {
    // 1. Check：同名用户已存在 → 核对密码（不覆盖）。
    let existing: Option<(String,)> = db_fetch_optional!(
        state.db,
        (String,),
        "SELECT password_hash FROM iam_users WHERE username = ? AND tenant_id = ? AND deleted_at IS NULL",
        &creds.username,
        DEFAULT_TENANT_ID
    )
    .map_err(|_| StepFailure::Failed("admin_check_db_error"))?;
    if let Some((hash,)) = existing {
        let ok = digua_common_crypto::password::verify(&creds.password, &hash)
            .map_err(|_| StepFailure::Failed("password_verify_error"))?;
        return if ok {
            Ok(()) // 幂等恢复：上次已创建成功但步骤状态未持久化。
        } else {
            Err(StepFailure::Conflict("admin username already taken".into()))
        };
    }

    // 2. 平台非空保护：已有其他用户 → 冲突（Bootstrap 首管理员要求空表）。
    let (user_count,): (i64,) = db_fetch_one!(
        state.db,
        (i64,),
        "SELECT COUNT(*) FROM iam_users WHERE deleted_at IS NULL"
    )
    .map_err(|_| StepFailure::Failed("admin_count_db_error"))?;
    if user_count > 0 {
        return Err(StepFailure::Conflict(
            "platform already has users; bootstrap admin requires an empty user table".into(),
        ));
    }

    // 3. Insert（本地事务：用户行 + IamUserCreated outbox 同事务）；
    //    唯一约束兜底命中 → 复检。
    let user_id = nanoid::nanoid!();
    let salt = nanoid::nanoid!(32);
    let password_hash = digua_common_crypto::password::hash(&creds.password)
        .map_err(|_| StepFailure::Failed("password_hash_error"))?;
    let nickname = creds
        .nickname
        .clone()
        .unwrap_or_else(|| creds.username.clone());

    if let Err(e) =
        insert_admin_user_tx(state, &user_id, creds, &password_hash, &salt, &nickname).await
    {
        if is_duplicate_key(&e) {
            return recheck_admin_user(state, creds).await;
        }
        tracing::error!(error = %e, "bootstrap admin insert failed");
        return Err(StepFailure::Failed("admin insert failed"));
    }

    // 4. Verify：回读并核对凭据属性。
    recheck_admin_user(state, creds).await
}

async fn recheck_admin_user(state: &AppState, creds: &AdminCredentials) -> Result<(), StepFailure> {
    let row: Option<(String,)> = db_fetch_optional!(
        state.db,
        (String,),
        "SELECT password_hash FROM iam_users WHERE username = ? AND tenant_id = ? AND deleted_at IS NULL",
        &creds.username,
        DEFAULT_TENANT_ID
    )
    .map_err(|_| StepFailure::Failed("admin_recheck_db_error"))?;
    match row {
        Some((hash,)) => {
            let ok = digua_common_crypto::password::verify(&creds.password, &hash)
                .map_err(|_| StepFailure::Failed("password_verify_error"))?;
            if ok {
                Ok(())
            } else {
                Err(StepFailure::Conflict("admin username already taken".into()))
            }
        }
        None => Err(StepFailure::Failed("admin row missing after insert")),
    }
}

/// 用户 INSERT + IamUserCreated outbox 同事务（双方言臂，prue 样板）。
async fn insert_admin_user_tx(
    state: &AppState,
    user_id: &str,
    creds: &AdminCredentials,
    password_hash: &str,
    salt: &str,
    nickname: &str,
) -> Result<(), sqlx::Error> {
    let email = creds.email.clone();
    match &state.db {
        #[cfg(feature = "mysql")]
        digua_common_db::DbPool::Mysql(p) => {
            let mut tx = p.begin().await?;
            sqlx::query(
                "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, email, phone, avatar, status, email_verified, phone_verified, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL, 'active', false, false, NOW(), NOW())",
            )
            .bind(user_id)
            .bind(DEFAULT_TENANT_ID)
            .bind(&creds.username)
            .bind(password_hash)
            .bind(salt)
            .bind(nickname)
            .bind(&email)
            .execute(&mut *tx)
            .await?;
            enqueue_user_created(
                state,
                &mut AnyTx::Mysql(&mut tx),
                user_id,
                &creds.username,
                &email,
                DEFAULT_TENANT_ID,
            )
            .await?;
            tx.commit().await
        }
        #[cfg(feature = "postgres")]
        digua_common_db::DbPool::Postgres(p) => {
            let mut tx = p.begin().await?;
            sqlx::query(
                "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, email, phone, avatar, status, email_verified, phone_verified, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, NULL, 'active', false, false, NOW(), NOW())",
            )
            .bind(user_id)
            .bind(DEFAULT_TENANT_ID)
            .bind(&creds.username)
            .bind(password_hash)
            .bind(salt)
            .bind(nickname)
            .bind(&email)
            .execute(&mut *tx)
            .await?;
            enqueue_user_created(
                state,
                &mut AnyTx::Postgres(&mut tx),
                user_id,
                &creds.username,
                &email,
                DEFAULT_TENANT_ID,
            )
            .await?;
            tx.commit().await
        }
        #[allow(unreachable_patterns)]
        _ => Err(sqlx::Error::Configuration(
            "bootstrap: dialect not enabled".into(),
        )),
    }
}

async fn enqueue_user_created(
    state: &AppState,
    tx: &mut AnyTx<'_, '_>,
    user_id: &str,
    username: &str,
    email: &Option<String>,
    tenant_id: i64,
) -> Result<(), sqlx::Error> {
    let mut envelope = Envelope::new(
        "iam-service",
        IamUserCreated {
            user_id: user_id.to_string(),
            username: username.to_string(),
            email: email.clone(),
            org_unit_id: None,
        },
    )
    .with_tenant_id(tenant_id.to_string());
    state
        .outbox_repo
        .enqueue(tx, &mut envelope)
        .await
        .map_err(|e| sqlx::Error::Configuration(Box::new(std::io::Error::other(e.to_string()))))
}

// ── S03 / S04：首管理员角色绑定（platform_admin + tenant_admin）──────────

pub async fn run_admin_platform_role(state: &AppState, user_id: &str) -> Result<(), StepFailure> {
    // Check：seed 角色必须存在且 active（V005 保证；被人工禁用 → 冲突不覆盖）。
    let row: Option<(String,)> = db_fetch_optional!(
        state.db,
        (String,),
        "SELECT status FROM iam_roles WHERE id = ? AND deleted_at IS NULL",
        PLATFORM_ADMIN_ROLE_ID
    )
    .map_err(|_| StepFailure::Failed("role_check_db_error"))?;
    match row {
        Some((status,)) if status == "active" => {
            bind_admin_role(state, user_id, PLATFORM_ADMIN_ROLE_ID).await
        }
        Some(_) => Err(StepFailure::Conflict(
            "platform_admin role is disabled".into(),
        )),
        None => Err(StepFailure::Failed("platform_admin role missing")),
    }
}

pub async fn run_admin_tenant_role(state: &AppState, user_id: &str) -> Result<(), StepFailure> {
    // Check：tenant_admin 为 V003 播种的 (tenant 1) 全局共享角色。
    let row: Option<(String,)> = db_fetch_optional!(
        state.db,
        (String,),
        "SELECT id FROM iam_roles WHERE tenant_id = ? AND code = 'tenant_admin' AND deleted_at IS NULL LIMIT 1",
        DEFAULT_TENANT_ID
    )
    .map_err(|_| StepFailure::Failed("role_check_db_error"))?;
    let Some((role_id,)) = row else {
        return Err(StepFailure::Failed("tenant_admin role missing"));
    };
    bind_admin_role(state, user_id, &role_id).await
}

/// 绑定 + role_assigned outbox 同事务；唯一约束冲突 → 复检视为已完成。
async fn bind_admin_role(
    state: &AppState,
    user_id: &str,
    role_id: &str,
) -> Result<(), StepFailure> {
    // Check：已绑定 → 幂等完成。
    let (bound,): (i64,) = db_fetch_one!(
        state.db,
        (i64,),
        "SELECT COUNT(*) FROM iam_user_roles WHERE user_id = ? AND role_id = ?",
        user_id,
        role_id
    )
    .map_err(|_| StepFailure::Failed("binding_check_db_error"))?;
    if bound > 0 {
        return Ok(());
    }

    match insert_binding_sql(state, user_id, role_id).await {
        BindingOutcome::Inserted | BindingOutcome::AlreadyBound => Ok(()),
        BindingOutcome::Failed(code) => Err(StepFailure::Failed(code)),
    }
}

enum BindingOutcome {
    Inserted,
    /// 唯一约束兜底命中（并发恢复竞态），复检已存在。
    AlreadyBound,
    Failed(&'static str),
}

async fn insert_binding_sql(state: &AppState, user_id: &str, role_id: &str) -> BindingOutcome {
    let ts = chrono::Utc::now().timestamp();
    match &state.db {
        #[cfg(feature = "mysql")]
        digua_common_db::DbPool::Mysql(p) => {
            let mut tx = match p.begin().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!(error = %e, "bootstrap binding tx begin failed");
                    return BindingOutcome::Failed("binding tx begin failed");
                }
            };
            if let Err(e) = sqlx::query(
                "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())",
            )
            .bind(user_id)
            .bind(role_id)
            .execute(&mut *tx)
            .await
            {
                if is_duplicate_key(&e) {
                    return BindingOutcome::AlreadyBound;
                }
                tracing::error!(error = %e, "bootstrap binding insert failed");
                return BindingOutcome::Failed("binding insert failed");
            }
            if let Err(e) =
                emit_role_assigned(state, &mut AnyTx::Mysql(&mut tx), user_id, role_id, ts).await
            {
                tracing::error!(error = %e, "bootstrap role_assigned outbox failed");
                return BindingOutcome::Failed("binding outbox failed");
            }
            match tx.commit().await {
                Ok(()) => BindingOutcome::Inserted,
                Err(e) => {
                    tracing::error!(error = %e, "bootstrap binding commit failed");
                    BindingOutcome::Failed("binding commit failed")
                }
            }
        }
        #[cfg(feature = "postgres")]
        digua_common_db::DbPool::Postgres(p) => {
            let mut tx = match p.begin().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!(error = %e, "bootstrap binding tx begin failed");
                    return BindingOutcome::Failed("binding tx begin failed");
                }
            };
            if let Err(e) = sqlx::query(
                "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES ($1, $2, NOW())",
            )
            .bind(user_id)
            .bind(role_id)
            .execute(&mut *tx)
            .await
            {
                if is_duplicate_key(&e) {
                    return BindingOutcome::AlreadyBound;
                }
                tracing::error!(error = %e, "bootstrap binding insert failed");
                return BindingOutcome::Failed("binding insert failed");
            }
            if let Err(e) =
                emit_role_assigned(state, &mut AnyTx::Postgres(&mut tx), user_id, role_id, ts).await
            {
                tracing::error!(error = %e, "bootstrap role_assigned outbox failed");
                return BindingOutcome::Failed("binding outbox failed");
            }
            match tx.commit().await {
                Ok(()) => BindingOutcome::Inserted,
                Err(e) => {
                    tracing::error!(error = %e, "bootstrap binding commit failed");
                    BindingOutcome::Failed("binding commit failed")
                }
            }
        }
        #[allow(unreachable_patterns)]
        _ => BindingOutcome::Failed("dialect not enabled"),
    }
}

/// role_assigned outbox 事件（绑定与事件同事务，consumer 据此刷新
/// perm_epoch —— 与 user_svc.create 初始绑定路径保持一致）。
async fn emit_role_assigned(
    state: &AppState,
    tx: &mut AnyTx<'_, '_>,
    user_id: &str,
    role_id: &str,
    ts: i64,
) -> Result<(), IamError> {
    crate::service::membership_event::enqueue_role_assigned(
        tx,
        state.outbox_repo.as_ref(),
        user_id,
        role_id,
        DEFAULT_TENANT_ID,
        None,
        ts,
    )
    .await
}

// ── 恢复判定辅助 ────────────────────────────────────────────────────────

/// 首管理员凭据是否必填（步骤未完成 → 必须重新提交；完成 → 不需要）。
pub fn credentials_needed(first_admin_completed: bool) -> bool {
    !first_admin_completed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_credentials_reject_short_password() {
        let req = BootstrapRequest {
            username: Some("admin".into()),
            password: Some("short".into()),
            email: None,
            nickname: None,
        };
        assert!(AdminCredentials::from_request(&req).is_err());
    }

    #[test]
    fn admin_credentials_reject_bad_username() {
        let req = BootstrapRequest {
            username: Some("bad name!".into()),
            password: Some("LongEnough1!".into()),
            email: None,
            nickname: None,
        };
        assert!(AdminCredentials::from_request(&req).is_err());
    }

    #[test]
    fn admin_credentials_reject_missing_fields() {
        let req = BootstrapRequest {
            username: None,
            password: Some("LongEnough1!".into()),
            email: None,
            nickname: None,
        };
        assert!(AdminCredentials::from_request(&req).is_err());
    }

    #[test]
    fn admin_credentials_accept_valid_input() {
        let req = BootstrapRequest {
            username: Some("admin".into()),
            password: Some("LongEnough1!".into()),
            email: Some("a@b.c".into()),
            nickname: None,
        };
        let creds = AdminCredentials::from_request(&req).expect("valid");
        let creds = creds.expect("some credentials");
        assert_eq!(creds.username, "admin");
        assert_eq!(creds.nickname, None);
    }

    #[test]
    fn duplicate_code_detection() {
        assert!(is_duplicate_code(Some("1062"), "Duplicate entry 'x'"));
        assert!(is_duplicate_code(
            Some("23505"),
            "duplicate key value violates unique constraint"
        ));
        assert!(!is_duplicate_code(Some("1064"), "syntax error"));
        assert!(!is_duplicate_code(None, "connection refused"));
    }

    #[test]
    fn credentials_needed_semantics() {
        assert!(credentials_needed(false));
        assert!(!credentials_needed(true));
    }
}
