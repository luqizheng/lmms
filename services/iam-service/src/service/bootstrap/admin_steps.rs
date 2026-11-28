//! Bootstrap IAM 本地步骤（S01~S04）：默认租户校验 + 首管理员及其双角色绑定。
//!
//! 每个 Step 遵循设计 Q6 统一语义：
//! Check（对象是否已存在）→ Insert（不存在才创建）→ Verify（核对关键
//! 属性）→ 报告结果。对象已存在且属性一致 → 视为幂等恢复成功；存在但
//! 属性不一致 → 报告冲突，**不得覆盖**。数据库唯一约束是防重复创建的
//! 最终保障（不依赖应用层 SELECT）。
//!
//! **执行权原子边界（WP-BOOTSTRAP-01-R1 / P1-02）**：S02/S03/S04 的业务
//! 写入统一走事务协议——`FOR UPDATE` 锁定 bootstrap request 行 → 锁后
//! 校验 owner/epoch/status/lease（`executor::lock_and_verify_in_tx`）→
//! 同事务内业务写入 → 提交。互斥证明见该函数 doc。每个 Step 仍是独立
//! 事务、独立提交、独立恢复；不把整个 Bootstrap 变成长事务。
//!
//! **首管理员唯一性（R1 / 4.4）**：S02 在业务事务内原子认领
//! `admin_user_id`（`executor::claim_admin_in_tx`，行已锁定 + 列单值 +
//! 唯一索引 `uk_iam_bootstrap_admin_user`）——同一 run 只能绑定一个首
//! 管理员；不同用户名并发执行者被 request 行锁串行化后命中"已认领"
//! fail-closed 分支。S03/S04 按认领的 user id 绑定（**无凭据恢复**，
//! 设计 Q7：S02 完成后不重提凭据），不再依赖本次请求提交的用户名/密码。
//!
//! IAM 本地步骤各自使用本地事务；不假设与 Token Service 共享事务。

use digua_common_db::{db_fetch_optional, DbPool};
use digua_common_event::events::IamUserCreated;
use digua_common_event::AnyTx;
use digua_common_event::{Envelope, OutboxRepo};

use crate::dto::BootstrapRequest;
use crate::error::IamError;
use crate::state::AppState;

use super::executor::{self, ExecutionHandle, VerifyFailure};

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

impl StepFailure {
    /// 执行权校验失败 → fencing 类失败映射为 Conflict（调用方立即让位），
    /// 记录级异常映射为 Failed。
    pub fn from_verify(f: &VerifyFailure) -> Self {
        tracing::warn!(verify = ?f, "bootstrap execution authority check failed in business tx");
        match f {
            VerifyFailure::Missing => {
                StepFailure::Failed("bootstrap request missing in business tx")
            }
            VerifyFailure::OwnerMismatch | VerifyFailure::EpochMismatch => {
                StepFailure::Conflict("bootstrap execution lost to a newer epoch".into())
            }
            VerifyFailure::StatusNotAllowed(_) => {
                StepFailure::Failed("bootstrap status not runnable in business tx")
            }
            VerifyFailure::LeaseExpired => {
                StepFailure::Conflict("bootstrap lease expired during business tx".into())
            }
        }
    }
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

/// S02 成功结果：认领（或确认）的首管理员 user id。
#[derive(Debug, Clone)]
pub struct FirstAdminOutcome {
    pub user_id: String,
}

/// S02/S03/S04 事务体的中间结果（收尾统一 commit/rollback）。
enum AdminTxOutcome {
    /// 成功：认领（或确认）的 user id。
    Done(String),
    Conflict(String),
    Failed(&'static str),
}

/// 事务收尾：Conflict/Failed 显式回滚，Done 提交（drop 兜底回滚仍在）。
async fn finish_admin_tx(
    tx: digua_common_db::Transaction<'_>,
    outcome: AdminTxOutcome,
) -> Result<String, StepFailure> {
    match outcome {
        AdminTxOutcome::Done(user_id) => match tx.commit().await {
            Ok(()) => Ok(user_id),
            Err(e) => {
                tracing::error!(error = %e, "bootstrap business tx commit failed");
                Err(StepFailure::Failed("business tx commit failed"))
            }
        },
        AdminTxOutcome::Conflict(msg) => {
            let _ = tx.rollback().await;
            Err(StepFailure::Conflict(msg))
        }
        AdminTxOutcome::Failed(code) => {
            let _ = tx.rollback().await;
            Err(StepFailure::Failed(code))
        }
    }
}

fn verify_outcome(
    r: Result<super::executor::VerifiedRow, VerifyFailure>,
) -> Result<super::executor::VerifiedRow, AdminTxOutcome> {
    match r {
        Ok(v) => Ok(v),
        Err(f) => Err(match StepFailure::from_verify(&f) {
            StepFailure::Conflict(m) => AdminTxOutcome::Conflict(m),
            StepFailure::Failed(c) => AdminTxOutcome::Failed(c),
            StepFailure::Dependency(_) => AdminTxOutcome::Failed("unexpected dependency"),
        }),
    }
}

// ── S01：默认租户存在性校验（Check-only，无业务写入 → 无需事务边界）──────

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

// ── S02：首管理员用户创建（事务协议：锁 request 行 → 校验 → 写入）────────

pub async fn run_first_admin_user(
    state: &AppState,
    handle: &ExecutionHandle,
    creds: &AdminCredentials,
) -> Result<FirstAdminOutcome, StepFailure> {
    // 哈希在事务外预计算（argon2 慢，不占 DB 事务时长/行锁窗口）。
    let password_hash = digua_common_crypto::password::hash(&creds.password)
        .map_err(|_| StepFailure::Failed("password_hash_error"))?;
    let salt = nanoid::nanoid!(32);
    let nickname = creds
        .nickname
        .clone()
        .unwrap_or_else(|| creds.username.clone());

    match &state.db {
        #[cfg(feature = "mysql")]
        digua_common_db::DbPool::Mysql(p) => {
            let sqlx_tx = p
                .begin()
                .await
                .map_err(|_| StepFailure::Failed("admin tx begin failed"))?;
            let mut tx = digua_common_db::Transaction::Mysql(sqlx_tx);
            let outcome = first_admin_mysql(
                &mut MysqlStmtTx(&mut tx),
                state,
                handle,
                creds,
                &password_hash,
                &salt,
                &nickname,
            )
            .await;
            finish_admin_tx(tx, outcome)
                .await
                .map(|user_id| FirstAdminOutcome { user_id })
        }
        #[cfg(feature = "postgres")]
        digua_common_db::DbPool::Postgres(p) => {
            let sqlx_tx = p
                .begin()
                .await
                .map_err(|_| StepFailure::Failed("admin tx begin failed"))?;
            let mut tx = digua_common_db::Transaction::Postgres(sqlx_tx);
            let outcome = first_admin_pg(
                &mut PgStmtTx(&mut tx),
                state,
                handle,
                creds,
                &password_hash,
                &salt,
                &nickname,
            )
            .await;
            finish_admin_tx(tx, outcome)
                .await
                .map(|user_id| FirstAdminOutcome { user_id })
        }
        #[allow(unreachable_patterns)]
        _ => Err(StepFailure::Failed("dialect not enabled")),
    }
}

/// S02 共享业务逻辑（方言分派由 `Stmt` 执行器处理；L1/V2/W3 语义见模块
/// doc）。参数 `st` 提供方言感知的语句执行。
async fn first_admin_body<S: StmtTx>(
    st: &mut S,
    state: &AppState,
    handle: &ExecutionHandle,
    creds: &AdminCredentials,
    password_hash: &str,
    salt: &str,
    nickname: &str,
) -> AdminTxOutcome {
    // L1 + V2：锁定 request 行并校验执行权（含 lease，锁后 DB 时钟）。
    let verified = match st.lock_and_verify(handle).await {
        Ok(r) => r,
        Err(_) => return AdminTxOutcome::Failed("authority verify db error"),
    };
    let verified = match verify_outcome(verified) {
        Ok(v) => v,
        Err(e) => return e,
    };

    // ── 分支 A：已认领（上次执行认领了但 S02 状态未持久化，或历史数据）──
    //    凭据必须匹配已认领身份，不一致 → fail-closed（不覆盖不重建）。
    if let Some(claimed_id) = verified.admin_user_id.clone() {
        return claimed_admin_check(st, creds, &claimed_id).await;
    }

    // ── 分支 B：未认领。Check：同名用户已存在 → 核对密码（不覆盖）。────
    let existing: Option<(String, String)> = match st
        .query_two_cols(
            "SELECT id, password_hash FROM iam_users WHERE username = ? AND tenant_id = ? AND deleted_at IS NULL",
            vec![bind_str(&creds.username), Bind::I64(DEFAULT_TENANT_ID)],
        )
        .await
    {
        Ok(v) => v.into_iter().next(),
        Err(_) => return AdminTxOutcome::Failed("admin_check_db_error"),
    };
    if let Some((existing_id, existing_hash)) = existing {
        let ok = match digua_common_crypto::password::verify(&creds.password, &existing_hash) {
            Ok(v) => v,
            Err(_) => return AdminTxOutcome::Failed("password_verify_error"),
        };
        if !ok {
            return AdminTxOutcome::Conflict("admin username already taken".into());
        }
        // 幂等恢复：同名+密码验证通过 → 补认领（历史数据兼容：R1 之前
        // 创建的用户没有 admin_user_id；密码学验证通过即归属确认，不是
        // 猜测——密码只有部署者知道）。
        if !st.claim_admin(handle, &existing_id).await {
            return AdminTxOutcome::Conflict(
                "admin claim conflict; manual recovery required".into(),
            );
        }
        return AdminTxOutcome::Done(existing_id);
    }

    // 平台非空保护：已有其他用户 → 冲突（Bootstrap 首管理员要求空表）。
    let count_rows: Vec<i64> = match st
        .query_i64(
            "SELECT COUNT(*) FROM iam_users WHERE deleted_at IS NULL",
            vec![],
        )
        .await
    {
        Ok(v) => v,
        Err(_) => return AdminTxOutcome::Failed("admin_count_db_error"),
    };
    if count_rows.first().copied().unwrap_or(1) > 0 {
        return AdminTxOutcome::Conflict(
            "platform already has users; bootstrap admin requires an empty user table".into(),
        );
    }

    // Insert：新用户行（id 由 nanoid 生成；username 唯一约束兜底并发）。
    let user_id = nanoid::nanoid!();
    if let Err(e) = st
        .insert_admin_user(
            &user_id,
            &creds.username,
            password_hash,
            salt,
            nickname,
            creds.email.as_deref(),
        )
        .await
    {
        if is_duplicate_key(&e) {
            // 唯一约束兜底命中：并发插入同名（行锁下理论不可达，防御）。
            return AdminTxOutcome::Conflict("admin username already taken".into());
        }
        tracing::error!(error = %e, "bootstrap admin insert failed");
        return AdminTxOutcome::Failed("admin insert failed");
    }

    // IamUserCreated outbox（同事务）。
    if let Err(e) = st
        .enqueue_user_created(state, &user_id, &creds.username, creds.email.as_deref())
        .await
    {
        tracing::error!(error = %e, "bootstrap user-created outbox failed");
        return AdminTxOutcome::Failed("admin outbox failed");
    }

    // W3：原子认领（行已锁定；`admin_user_id IS NULL` 条件兜底恢复路径）。
    if !st.claim_admin(handle, &user_id).await {
        return AdminTxOutcome::Conflict("admin claim conflict; manual recovery required".into());
    }

    // Verify：回读并核对凭据属性（密码哈希匹配）。
    let stored: Vec<String> = match st
        .query_str(
            "SELECT password_hash FROM iam_users WHERE id = ? AND deleted_at IS NULL",
            vec![bind_str(&user_id)],
        )
        .await
    {
        Ok(v) => v,
        Err(_) => return AdminTxOutcome::Failed("admin_recheck_db_error"),
    };
    let Some(stored_hash) = stored.into_iter().next() else {
        return AdminTxOutcome::Failed("admin row missing after insert");
    };
    let ok = match digua_common_crypto::password::verify(&creds.password, &stored_hash) {
        Ok(v) => v,
        Err(_) => return AdminTxOutcome::Failed("password_verify_error"),
    };
    if !ok {
        return AdminTxOutcome::Failed("admin verify failed");
    }
    AdminTxOutcome::Done(user_id)
}

/// 已认领分支：恢复者提交的凭据必须匹配已认领身份。
async fn claimed_admin_check<S: StmtTx>(
    st: &mut S,
    creds: &AdminCredentials,
    claimed_id: &str,
) -> AdminTxOutcome {
    let rows: Vec<(String, String)> = match st
        .query_two_cols(
            "SELECT username, password_hash FROM iam_users WHERE id = ? AND deleted_at IS NULL",
            vec![bind_str(claimed_id)],
        )
        .await
    {
        Ok(v) => v,
        Err(_) => return AdminTxOutcome::Failed("admin_check_db_error"),
    };
    let Some((username, hash)) = rows.into_iter().next() else {
        // 认领的用户已被删除：fail-closed，不自动重建替代用户。
        return AdminTxOutcome::Conflict(
            "claimed admin user missing; manual recovery required".into(),
        );
    };
    if username != creds.username {
        // 已有管理员身份不允许被另一个用户名静默替换。
        return AdminTxOutcome::Conflict(
            "admin identity already claimed by a different username".into(),
        );
    }
    let ok = match digua_common_crypto::password::verify(&creds.password, &hash) {
        Ok(v) => v,
        Err(_) => return AdminTxOutcome::Failed("password_verify_error"),
    };
    if ok {
        AdminTxOutcome::Done(claimed_id.to_string())
    } else {
        AdminTxOutcome::Conflict("admin username already taken".into())
    }
}

// ── 方言语句执行器：占位符按方言渲染（`?` ↔ `$N`），bind 顺序一致。──────

/// 绑定值（统一按字符串编码；sqlx 按 SQL 上下文解码，tenant_id 等数字列
/// 由 BIGINT 接收字符串在 MySQL 隐式转换 / PG 侧显式 cast）。
///
/// ⚠️ PG 侧 BIGINT 列不接受字符串 bind，因此数字 bind 在渲染层内联为
/// 字面量（`Bind::i64` 仅用于受控常量：DEFAULT_TENANT_ID），字符串 bind
/// 走参数化（防注入）。
#[derive(Clone)]
enum Bind {
    Str(String),
    I64(i64),
}

fn bind_str(s: &str) -> Bind {
    Bind::Str(s.to_string())
}

/// 方言感知的事务语句执行器（由两个方言臂各实现一份；SQL 文本统一写
/// `?` 占位符，PG 侧在执行器内改写为 `$N`——与 `digua_common_db::sql`
/// 的渲染契约一致，bind 顺序不变）。
trait StmtTx {
    /// L1+V2：FOR UPDATE 锁 request 行 + 校验执行权（复用 executor 原语，
    /// 内部用方言宏，双方言一致）。
    async fn lock_and_verify(
        &mut self,
        handle: &ExecutionHandle,
    ) -> Result<Result<super::executor::VerifiedRow, VerifyFailure>, sqlx::Error>;

    async fn query_str(&mut self, sql: &str, binds: Vec<Bind>) -> Result<Vec<String>, sqlx::Error>;
    async fn query_i64(&mut self, sql: &str, binds: Vec<Bind>) -> Result<Vec<i64>, sqlx::Error>;
    async fn query_two_cols(
        &mut self,
        sql: &str,
        binds: Vec<Bind>,
    ) -> Result<Vec<(String, String)>, sqlx::Error>;
    async fn execute(&mut self, sql: &str, binds: Vec<Bind>) -> Result<u64, sqlx::Error>;
    async fn claim_admin(&mut self, handle: &ExecutionHandle, user_id: &str) -> bool;
    async fn insert_admin_user(
        &mut self,
        user_id: &str,
        username: &str,
        password_hash: &str,
        salt: &str,
        nickname: &str,
        email: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn enqueue_user_created(
        &mut self,
        state: &AppState,
        user_id: &str,
        username: &str,
        email: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn enqueue_role_assigned(
        &mut self,
        state: &AppState,
        user_id: &str,
        role_id: &str,
        ts: i64,
    ) -> Result<(), IamError>;
}

// `?` → `$N` 渲染（PG 臂用）：按出现顺序编号，与 bind 顺序契约一致。
#[cfg(feature = "postgres")]
fn render_pg_placeholders(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len() + 8);
    let mut n = 0;
    for ch in sql.chars() {
        if ch == '?' {
            n += 1;
            out.push('$');
            out.push_str(&n.to_string());
        } else {
            out.push(ch);
        }
    }
    out
}

// ── S03 / S04：首管理员角色绑定（按认领的 admin_user_id，无凭据）─────────

pub async fn run_admin_platform_role(
    state: &AppState,
    handle: &ExecutionHandle,
) -> Result<(), StepFailure> {
    // Check：seed 角色必须存在且 active（V005 保证；被人工禁用 → 冲突不覆盖）。
    // 角色存在性是只读 Check，无需事务边界；绑定写入走事务协议。
    let row: Option<(String,)> = db_fetch_optional!(
        state.db,
        (String,),
        "SELECT status FROM iam_roles WHERE id = ? AND deleted_at IS NULL",
        PLATFORM_ADMIN_ROLE_ID
    )
    .map_err(|_| StepFailure::Failed("role_check_db_error"))?;
    match row {
        Some((status,)) if status == "active" => {
            bind_admin_role(state, handle, PLATFORM_ADMIN_ROLE_ID).await
        }
        Some(_) => Err(StepFailure::Conflict(
            "platform_admin role is disabled".into(),
        )),
        None => Err(StepFailure::Failed("platform_admin role missing")),
    }
}

pub async fn run_admin_tenant_role(
    state: &AppState,
    handle: &ExecutionHandle,
) -> Result<(), StepFailure> {
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
    bind_admin_role(state, handle, &role_id).await
}

/// 绑定事务：锁 request 行 → 校验执行权 → 解析认领管理员（fail-closed）→
/// Check 用户存在/绑定存在 → INSERT 绑定 + role_assigned outbox → COMMIT。
async fn bind_admin_role(
    state: &AppState,
    handle: &ExecutionHandle,
    role_id: &str,
) -> Result<(), StepFailure> {
    match &state.db {
        #[cfg(feature = "mysql")]
        digua_common_db::DbPool::Mysql(p) => {
            let sqlx_tx = p
                .begin()
                .await
                .map_err(|_| StepFailure::Failed("binding tx begin failed"))?;
            let mut tx = digua_common_db::Transaction::Mysql(sqlx_tx);
            let outcome = binding_body(&mut MysqlStmtTx(&mut tx), state, handle, role_id).await;
            finish_admin_tx(tx, outcome).await.map(|_| ())
        }
        #[cfg(feature = "postgres")]
        digua_common_db::DbPool::Postgres(p) => {
            let sqlx_tx = p
                .begin()
                .await
                .map_err(|_| StepFailure::Failed("binding tx begin failed"))?;
            let mut tx = digua_common_db::Transaction::Postgres(sqlx_tx);
            let outcome = binding_body(&mut PgStmtTx(&mut tx), state, handle, role_id).await;
            finish_admin_tx(tx, outcome).await.map(|_| ())
        }
        #[allow(unreachable_patterns)]
        _ => Err(StepFailure::Failed("dialect not enabled")),
    }
}

async fn binding_body<S: StmtTx>(
    st: &mut S,
    state: &AppState,
    handle: &ExecutionHandle,
    role_id: &str,
) -> AdminTxOutcome {
    // L1 + V2：锁定 request 行并校验执行权。
    let verified = match st.lock_and_verify(handle).await {
        Ok(r) => r,
        Err(_) => return AdminTxOutcome::Failed("authority verify db error"),
    };
    let verified = match verify_outcome(verified) {
        Ok(v) => v,
        Err(e) => return e,
    };

    // 解析认领的首管理员：列缺失 → fail-closed（不猜测身份，明确的人工
    // 处理路径；错误摘要进入 last_error 供操作员定位）。
    let Some(user_id) = verified.admin_user_id else {
        return AdminTxOutcome::Conflict(
            "admin claim missing for role binding; manual recovery required".into(),
        );
    };

    // Check：绑定目标用户必须存在且未删除（fail-closed，不自动重建）。
    let exists: Vec<String> = match st
        .query_str(
            "SELECT id FROM iam_users WHERE id = ? AND deleted_at IS NULL",
            vec![bind_str(&user_id)],
        )
        .await
    {
        Ok(v) => v,
        Err(_) => return AdminTxOutcome::Failed("binding_check_db_error"),
    };
    if exists.is_empty() {
        return AdminTxOutcome::Failed("claimed admin user missing");
    }

    // Check：已绑定 → 幂等完成。
    let bound: Vec<i64> = match st
        .query_i64(
            "SELECT COUNT(*) FROM iam_user_roles WHERE user_id = ? AND role_id = ?",
            vec![bind_str(&user_id), Bind::Str(role_id.to_string())],
        )
        .await
    {
        Ok(v) => v,
        Err(_) => return AdminTxOutcome::Failed("binding_check_db_error"),
    };
    if bound.first().copied().unwrap_or(1) > 0 {
        return AdminTxOutcome::Done(user_id);
    }

    // Insert 绑定（PK (user_id, role_id) 唯一约束兜底并发恢复）。
    if let Err(e) = st
        .execute(
            "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())",
            vec![bind_str(&user_id), Bind::Str(role_id.to_string())],
        )
        .await
    {
        if is_duplicate_key(&e) {
            return AdminTxOutcome::Done(user_id);
        }
        tracing::error!(error = %e, "bootstrap binding insert failed");
        return AdminTxOutcome::Failed("binding insert failed");
    }

    // role_assigned outbox（同事务，consumer 刷新 perm_epoch）。
    let ts = chrono::Utc::now().timestamp();
    if let Err(e) = st.enqueue_role_assigned(state, &user_id, role_id, ts).await {
        tracing::error!(error = %e, "bootstrap role_assigned outbox failed");
        return AdminTxOutcome::Failed("binding outbox failed");
    }
    AdminTxOutcome::Done(user_id)
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

/// 首管理员凭据是否必填（S02 未完成 → 必须重新提交；完成 → 不需要——
/// S03/S04 按持久化的 admin_user_id 绑定，与凭据无关）。
pub fn credentials_needed(first_admin_completed: bool) -> bool {
    !first_admin_completed
}

// 引用点：模块级引用保持公开符号被使用。
pub use super::steps::STEP_FIRST_ADMIN_USER as FIRST_ADMIN_STEP_KEY;

// ── MySQL 方言臂 ────────────────────────────────────────────────────────

#[cfg(feature = "mysql")]
struct MysqlStmtTx<'a, 'b>(&'b mut digua_common_db::Transaction<'a>);

#[cfg(feature = "mysql")]
impl<'a> MysqlStmtTx<'a, '_> {
    fn sqlx_tx(&mut self) -> &mut sqlx::Transaction<'a, sqlx::MySql> {
        match &mut *self.0 {
            digua_common_db::Transaction::Mysql(t) => t,
            #[allow(unreachable_patterns)]
            _ => unreachable!("mysql stmt tx"),
        }
    }
}

#[cfg(feature = "mysql")]
#[cfg(feature = "mysql")]
impl StmtTx for MysqlStmtTx<'_, '_> {
    async fn lock_and_verify(
        &mut self,
        handle: &ExecutionHandle,
    ) -> Result<Result<super::executor::VerifiedRow, VerifyFailure>, sqlx::Error> {
        executor::lock_and_verify_in_tx(&mut *self.0, handle).await
    }

    async fn query_str(&mut self, sql: &str, binds: Vec<Bind>) -> Result<Vec<String>, sqlx::Error> {
        let mut q = sqlx::query_as::<sqlx::MySql, (String,)>(sql);
        for b in binds {
            q = match b {
                Bind::Str(sv) => q.bind(sv),
                Bind::I64(v) => q.bind(v),
            };
        }
        Ok(q.fetch_all(&mut **self.sqlx_tx())
            .await?
            .into_iter()
            .map(|(v,)| v)
            .collect())
    }

    async fn query_i64(&mut self, sql: &str, binds: Vec<Bind>) -> Result<Vec<i64>, sqlx::Error> {
        let mut q = sqlx::query_as::<sqlx::MySql, (i64,)>(sql);
        for b in binds {
            q = match b {
                Bind::Str(sv) => q.bind(sv),
                Bind::I64(v) => q.bind(v),
            };
        }
        Ok(q.fetch_all(&mut **self.sqlx_tx())
            .await?
            .into_iter()
            .map(|(v,)| v)
            .collect())
    }

    async fn query_two_cols(
        &mut self,
        sql: &str,
        binds: Vec<Bind>,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        let mut q = sqlx::query_as::<sqlx::MySql, (String, String)>(sql);
        for b in binds {
            q = match b {
                Bind::Str(sv) => q.bind(sv),
                Bind::I64(v) => q.bind(v),
            };
        }
        q.fetch_all(&mut **self.sqlx_tx()).await
    }

    async fn execute(&mut self, sql: &str, binds: Vec<Bind>) -> Result<u64, sqlx::Error> {
        let mut q = sqlx::query(sql);
        for b in binds {
            q = match b {
                Bind::Str(sv) => q.bind(sv),
                Bind::I64(v) => q.bind(v),
            };
        }
        Ok(q.execute(&mut **self.sqlx_tx()).await?.rows_affected())
    }

    async fn claim_admin(&mut self, handle: &ExecutionHandle, user_id: &str) -> bool {
        executor::claim_admin_in_tx(&mut *self.0, handle, user_id)
            .await
            .unwrap_or(false)
    }

    async fn insert_admin_user(
        &mut self,
        user_id: &str,
        username: &str,
        password_hash: &str,
        salt: &str,
        nickname: &str,
        email: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, email, phone, avatar, status, email_verified, phone_verified, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL, 'active', false, false, NOW(), NOW())",
        )
        .bind(user_id)
        .bind(DEFAULT_TENANT_ID)
        .bind(username)
        .bind(password_hash)
        .bind(salt)
        .bind(nickname)
        .bind(email)
        .execute(&mut **self.sqlx_tx())
        .await
        .map(|_| ())
    }

    async fn enqueue_user_created(
        &mut self,
        state: &AppState,
        user_id: &str,
        username: &str,
        email: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let mut envelope = Envelope::new(
            "iam-service",
            IamUserCreated {
                user_id: user_id.to_string(),
                username: username.to_string(),
                email: email.map(String::from),
                org_unit_id: None,
            },
        )
        .with_tenant_id(DEFAULT_TENANT_ID.to_string());
        state
            .outbox_repo
            .enqueue(&mut AnyTx::Mysql(self.sqlx_tx()), &mut envelope)
            .await
            .map_err(|e| sqlx::Error::Configuration(Box::new(std::io::Error::other(e.to_string()))))
    }

    async fn enqueue_role_assigned(
        &mut self,
        state: &AppState,
        user_id: &str,
        role_id: &str,
        ts: i64,
    ) -> Result<(), IamError> {
        emit_role_assigned(
            state,
            &mut AnyTx::Mysql(self.sqlx_tx()),
            user_id,
            role_id,
            ts,
        )
        .await
    }
}

// ── PostgreSQL 方言臂 ───────────────────────────────────────────────────

#[cfg(feature = "postgres")]
struct PgStmtTx<'a, 'b>(&'b mut digua_common_db::Transaction<'a>);

#[cfg(feature = "postgres")]
impl<'a> PgStmtTx<'a, '_> {
    fn sqlx_tx(&mut self) -> &mut sqlx::Transaction<'a, sqlx::Postgres> {
        match &mut *self.0 {
            digua_common_db::Transaction::Postgres(t) => t,
            #[allow(unreachable_patterns)]
            _ => unreachable!("pg stmt tx"),
        }
    }
}

#[cfg(feature = "postgres")]
#[cfg(feature = "postgres")]
impl StmtTx for PgStmtTx<'_, '_> {
    async fn lock_and_verify(
        &mut self,
        handle: &ExecutionHandle,
    ) -> Result<Result<super::executor::VerifiedRow, VerifyFailure>, sqlx::Error> {
        executor::lock_and_verify_in_tx(&mut *self.0, handle).await
    }

    async fn query_str(&mut self, sql: &str, binds: Vec<Bind>) -> Result<Vec<String>, sqlx::Error> {
        let sql = render_pg_placeholders(sql);
        let mut q = sqlx::query_as::<sqlx::Postgres, (String,)>(&sql);
        for b in binds {
            q = match b {
                Bind::Str(sv) => q.bind(sv),
                Bind::I64(v) => q.bind(v),
            };
        }
        Ok(q.fetch_all(&mut **self.sqlx_tx())
            .await?
            .into_iter()
            .map(|(v,)| v)
            .collect())
    }

    async fn query_i64(&mut self, sql: &str, binds: Vec<Bind>) -> Result<Vec<i64>, sqlx::Error> {
        let sql = render_pg_placeholders(sql);
        let mut q = sqlx::query_as::<sqlx::Postgres, (i64,)>(&sql);
        for b in binds {
            q = match b {
                Bind::Str(sv) => q.bind(sv),
                Bind::I64(v) => q.bind(v),
            };
        }
        Ok(q.fetch_all(&mut **self.sqlx_tx())
            .await?
            .into_iter()
            .map(|(v,)| v)
            .collect())
    }

    async fn query_two_cols(
        &mut self,
        sql: &str,
        binds: Vec<Bind>,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        let sql = render_pg_placeholders(sql);
        let mut q = sqlx::query_as::<sqlx::Postgres, (String, String)>(&sql);
        for b in binds {
            q = match b {
                Bind::Str(sv) => q.bind(sv),
                Bind::I64(v) => q.bind(v),
            };
        }
        q.fetch_all(&mut **self.sqlx_tx()).await
    }

    async fn execute(&mut self, sql: &str, binds: Vec<Bind>) -> Result<u64, sqlx::Error> {
        let sql = render_pg_placeholders(sql);
        let mut q = sqlx::query(&sql);
        for b in binds {
            q = match b {
                Bind::Str(sv) => q.bind(sv),
                Bind::I64(v) => q.bind(v),
            };
        }
        Ok(q.execute(&mut **self.sqlx_tx()).await?.rows_affected())
    }

    async fn claim_admin(&mut self, handle: &ExecutionHandle, user_id: &str) -> bool {
        executor::claim_admin_in_tx(&mut *self.0, handle, user_id)
            .await
            .unwrap_or(false)
    }

    async fn insert_admin_user(
        &mut self,
        user_id: &str,
        username: &str,
        password_hash: &str,
        salt: &str,
        nickname: &str,
        email: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, email, phone, avatar, status, email_verified, phone_verified, created_at, updated_at) \
             VALUES ($1, $2::bigint, $3, $4, $5, $6, $7, NULL, NULL, 'active', false, false, NOW(), NOW())",
        )
        .bind(user_id)
        .bind(DEFAULT_TENANT_ID)
        .bind(username)
        .bind(password_hash)
        .bind(salt)
        .bind(nickname)
        .bind(email)
        .execute(&mut **self.sqlx_tx())
        .await
        .map(|_| ())
    }

    async fn enqueue_user_created(
        &mut self,
        state: &AppState,
        user_id: &str,
        username: &str,
        email: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let mut envelope = Envelope::new(
            "iam-service",
            IamUserCreated {
                user_id: user_id.to_string(),
                username: username.to_string(),
                email: email.map(String::from),
                org_unit_id: None,
            },
        )
        .with_tenant_id(DEFAULT_TENANT_ID.to_string());
        state
            .outbox_repo
            .enqueue(&mut AnyTx::Postgres(self.sqlx_tx()), &mut envelope)
            .await
            .map_err(|e| sqlx::Error::Configuration(Box::new(std::io::Error::other(e.to_string()))))
    }

    async fn enqueue_role_assigned(
        &mut self,
        state: &AppState,
        user_id: &str,
        role_id: &str,
        ts: i64,
    ) -> Result<(), IamError> {
        emit_role_assigned(
            state,
            &mut AnyTx::Postgres(self.sqlx_tx()),
            user_id,
            role_id,
            ts,
        )
        .await
    }
}

// ── S02 方言臂接线（共享事务体 + 方言执行器）────────────────────────────

#[cfg(feature = "mysql")]
async fn first_admin_mysql(
    st: &mut MysqlStmtTx<'_, '_>,
    state: &AppState,
    handle: &ExecutionHandle,
    creds: &AdminCredentials,
    password_hash: &str,
    salt: &str,
    nickname: &str,
) -> AdminTxOutcome {
    first_admin_body(st, state, handle, creds, password_hash, salt, nickname).await
}

#[cfg(feature = "postgres")]
async fn first_admin_pg(
    st: &mut PgStmtTx<'_, '_>,
    state: &AppState,
    handle: &ExecutionHandle,
    creds: &AdminCredentials,
    password_hash: &str,
    salt: &str,
    nickname: &str,
) -> AdminTxOutcome {
    first_admin_body(st, state, handle, creds, password_hash, salt, nickname).await
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

    #[test]
    fn verify_failure_maps_to_conflict_for_fencing() {
        let f = StepFailure::from_verify(&VerifyFailure::EpochMismatch);
        assert!(matches!(f, StepFailure::Conflict(_)));
        let f = StepFailure::from_verify(&VerifyFailure::Missing);
        assert!(matches!(f, StepFailure::Failed(_)));
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_placeholder_rendering() {
        assert_eq!(
            render_pg_placeholders("SELECT a, b FROM t WHERE x = ? AND y = ?"),
            "SELECT a, b FROM t WHERE x = $1 AND y = $2"
        );
        assert_eq!(render_pg_placeholders("no placeholders"), "no placeholders");
    }
}
