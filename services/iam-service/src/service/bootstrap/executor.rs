//! Bootstrap 执行权：数据库原子 claim + owner/epoch fencing + 租约。
//!
//! 设计基线 Q5：请求仲裁（gate 选胜者）与执行权是两个机制。执行权通过
//! 数据库原子条件更新取得，`execution_epoch` 每次成功 claim 递增，作为
//! fencing token——所有步骤状态写入都必须携带 owner+epoch 条件，旧执行
//! 者在执行权被重新授予后无法继续写入。
//!
//! 不使用进程本地锁 / Mutex：跨实例互斥只由数据库保证。

use chrono::{DateTime, Duration, Utc};
use digua_common_db::{db_execute, db_fetch_optional, db_tx_fetch_optional, DbPool, Transaction};
use serde_json::Value;
use sqlx::Error;

use super::gate::{db_now, tx_now};

/// 执行租约时长（秒）。步骤失败时主动释放（Q7：释放或等待到期二选一，
/// 本实现选择主动释放以便用户立即恢复）。
pub const LEASE_SECS: i64 = 90;

/// 当前执行权句柄（fencing 三元组）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionHandle {
    pub request_id: i64,
    pub owner: String,
    pub epoch: i64,
}

/// 胜出请求行（steps JSON / 执行权字段）。
#[derive(Debug, sqlx::FromRow)]
pub struct RequestRow {
    pub id: i64,
    pub status: String,
    pub steps: Option<Value>,
    pub execution_owner: Option<String>,
    pub execution_epoch: i64,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

/// 业务事务内 `FOR UPDATE` 锁定 request 行后的校验结果。
#[derive(Debug, Clone)]
pub struct VerifiedRow {
    pub admin_user_id: Option<String>,
}

/// 业务事务内的执行权校验失败原因（摘要不含敏感信息）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyFailure {
    /// 行不存在（记录被删除）。
    Missing,
    /// owner 不匹配（另一实例持有执行权）。
    OwnerMismatch,
    /// epoch 不匹配（执行权已被重新授予——旧执行者被 fencing）。
    EpochMismatch,
    /// 总状态不允许执行（completed / rejected / collecting / selected）。
    StatusNotAllowed(String),
    /// 租约已过期（执行者异常延迟；等待显式恢复重新 claim）。
    LeaseExpired,
}

/// 按 id 读取请求记录。
pub async fn fetch_request(pool: &DbPool, id: i64) -> Result<Option<RequestRow>, Error> {
    db_fetch_optional!(
        pool,
        RequestRow,
        "SELECT id, status, steps, execution_owner, execution_epoch, lease_expires_at, last_error \
         FROM iam_bootstrap_init_requests WHERE id = ?",
        id
    )
}

/// 读取 gate 指定的胜出记录（含 gate 关闭前的历史 winner）。
pub async fn find_winner_request(
    pool: &DbPool,
    winner_request_id: Option<i64>,
) -> Result<Option<RequestRow>, Error> {
    match winner_request_id {
        Some(id) => fetch_request(pool, id).await,
        None => Ok(None),
    }
}

/// 原子取得执行权（设计 Q5 示意 SQL 的双方言实现）。
///
/// 成功条件（单条 UPDATE 原子判定，受影响行数 == 1）：
/// - 记录状态 ∈ {selected, failed, running}（rejected/completed/collecting 不可执行）；
/// - 租约空闲（NULL 或已过期）。
///
/// 成功即 `execution_epoch += 1`（fencing token 递增）并写入租约。
/// 业务事务内的执行权原子校验（WP-BOOTSTRAP-01-R1 / P1-02 原子边界）。
///
/// **事务锁协议**（调用方必须在同一事务内完成业务写入并提交/回滚）：
///
/// ```text
/// L1: SELECT ... FROM iam_bootstrap_init_requests WHERE id = ? FOR UPDATE
/// V2: 锁后校验 owner / epoch / status = 'running' / lease >= tx_now()
/// W3: 同事务内执行 Step 的 Check / Insert / Verify
/// COMMIT
/// ```
///
/// **互斥证明**：`claim_execution` / `renew_lease` / `mark_*` 全部是对同
/// 一 request 行的单条 UPDATE，都需要该行排他行锁。因此：
///
/// 1. 本事务 L1 持锁期间，恢复者的 claim（epoch+1）被阻塞——新执行者
///    不可能在旧执行者的业务事务仍能合法提交时取得执行权；
/// 2. 恢复者先 claim 成功后，本事务的 L1 锁定读（MySQL 当前读 / PG
///    READ COMMITTED）读到最新已提交行 → epoch 不匹配 → 立即失败回滚，
///    无业务副作用；
/// 3. V2 校验在持锁期间恒成立（唯一能变更 owner/epoch/lease 的路径都
///    需要行锁），提交即"持有效执行权时的副作用"，无需二次校验；
/// 4. 所有路径锁序一致（request 行 → 业务表），无反向锁序死锁。
///
/// 锁后 DB 时钟判定租约（`tx_now`，不用本地时钟）；过期 → `LeaseExpired`
/// （fail-closed：等待用户显式恢复重新 claim，不自动续期）。
/// 锁定读的行形态（owner, epoch, status, lease, admin_user_id）。
type LockedAuthorityRow = (
    Option<String>,
    i64,
    String,
    Option<DateTime<Utc>>,
    Option<String>,
);

pub async fn lock_and_verify_in_tx(
    tx: &mut Transaction<'_>,
    handle: &ExecutionHandle,
) -> Result<Result<VerifiedRow, VerifyFailure>, Error> {
    let row: Option<LockedAuthorityRow> = db_tx_fetch_optional!(
        tx,
        (
            Option<String>,
            i64,
            String,
            Option<DateTime<Utc>>,
            Option<String>
        ),
        "SELECT execution_owner, execution_epoch, status, lease_expires_at, admin_user_id \
             FROM iam_bootstrap_init_requests WHERE id = ? FOR UPDATE",
        handle.request_id
    )?;
    let Some((owner, epoch, status, lease, admin_user_id)) = row else {
        return Ok(Err(VerifyFailure::Missing));
    };
    if owner.as_deref() != Some(handle.owner.as_str()) {
        return Ok(Err(VerifyFailure::OwnerMismatch));
    }
    if epoch != handle.epoch {
        return Ok(Err(VerifyFailure::EpochMismatch));
    }
    if status != "running" {
        return Ok(Err(VerifyFailure::StatusNotAllowed(status)));
    }
    let now = tx_now(tx).await?;
    match lease {
        Some(expires) if expires >= now => {}
        _ => return Ok(Err(VerifyFailure::LeaseExpired)),
    }
    Ok(Ok(VerifiedRow { admin_user_id }))
}

/// 在业务事务内原子认领首管理员（P1-01 / 4.4 唯一性保护）。
///
/// 前置条件：调用方已通过 [`lock_and_verify_in_tx`] 持有 request 行锁。
/// 行已锁定 → 无并发竞争；`admin_user_id IS NULL` 条件兜底"上次执行已
/// 认领但步骤状态未持久化"的恢复路径（此时返回 `AlreadyClaimed`，调用
/// 方走已认领分支，不得覆盖）。
pub async fn claim_admin_in_tx(
    tx: &mut Transaction<'_>,
    handle: &ExecutionHandle,
    user_id: &str,
) -> Result<bool, Error> {
    let now = tx_now(tx).await?;
    let n = digua_common_db::db_tx_execute!(
        tx,
        "UPDATE iam_bootstrap_init_requests SET admin_user_id = ?, updated_at = ? \
         WHERE id = ? AND admin_user_id IS NULL \
           AND execution_owner = ? AND execution_epoch = ?",
        user_id,
        now,
        handle.request_id,
        &handle.owner,
        handle.epoch
    )?;
    Ok(n == 1)
}

/// 读取认领的首管理员 user id（无凭据恢复路径）。
pub async fn claimed_admin_user_id(
    pool: &DbPool,
    request_id: i64,
) -> Result<Option<String>, Error> {
    let row: Option<(Option<String>,)> = db_fetch_optional!(
        pool,
        (Option<String>,),
        "SELECT admin_user_id FROM iam_bootstrap_init_requests WHERE id = ?",
        request_id
    )?;
    Ok(row.and_then(|(v,)| v))
}

pub async fn claim_execution(
    pool: &DbPool,
    request_id: i64,
    owner: &str,
) -> Result<Option<ExecutionHandle>, Error> {
    let now = db_now(pool).await?;
    let lease_end = now + Duration::seconds(LEASE_SECS);
    let n = db_execute!(
        pool,
        "UPDATE iam_bootstrap_init_requests \
         SET status = 'running', execution_owner = ?, execution_epoch = execution_epoch + 1, \
             lease_expires_at = ?, started_at = ?, updated_at = ? \
         WHERE id = ? \
           AND status IN ('selected', 'failed', 'running') \
           AND (lease_expires_at IS NULL OR lease_expires_at < ?)",
        owner,
        lease_end,
        now,
        now,
        request_id,
        now
    )?;
    if n != 1 {
        return Ok(None);
    }
    // 读回新 epoch：claim 的 UPDATE 已把 owner 置为本次调用方且租约在手，
    // 同 owner 读取不会看到别的 claimer 的 epoch。
    let row: Option<(i64,)> = db_fetch_optional!(
        pool,
        (i64,),
        "SELECT execution_epoch FROM iam_bootstrap_init_requests \
         WHERE id = ? AND execution_owner = ?",
        request_id,
        owner
    )?;
    Ok(row.map(|(epoch,)| ExecutionHandle {
        request_id,
        owner: owner.to_string(),
        epoch,
    }))
}

/// 续租（owner+epoch 条件更新；返回 false = 执行权已丢失）。
pub async fn renew_lease(pool: &DbPool, handle: &ExecutionHandle) -> Result<bool, Error> {
    let now = db_now(pool).await?;
    let lease_end = now + Duration::seconds(LEASE_SECS);
    let n = db_execute!(
        pool,
        "UPDATE iam_bootstrap_init_requests SET lease_expires_at = ?, updated_at = ? \
         WHERE id = ? AND execution_owner = ? AND execution_epoch = ? AND status = 'running'",
        lease_end,
        now,
        handle.request_id,
        &handle.owner,
        handle.epoch
    )?;
    Ok(n == 1)
}

/// 持久化步骤状态（fencing 写入：owner+epoch 条件，旧执行者写不进）。
///
/// 同时把 failed 总状态拉回 running（步骤重试中），并清空 last_error。
pub async fn persist_step_progress(
    pool: &DbPool,
    handle: &ExecutionHandle,
    steps: &Value,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let n = db_execute!(
        pool,
        "UPDATE iam_bootstrap_init_requests SET steps = ?, status = 'running', last_error = NULL, updated_at = ? \
         WHERE id = ? AND execution_owner = ? AND execution_epoch = ? \
           AND status IN ('running', 'selected', 'failed')",
        steps,
        now,
        handle.request_id,
        &handle.owner,
        handle.epoch
    )?;
    Ok(n == 1)
}

/// 标记单个步骤失败并释放租约（Q7：失败 → 总状态 failed，保留已完成
/// 步骤，释放租约等待用户显式恢复）。
pub async fn mark_step_failed(
    pool: &DbPool,
    handle: &ExecutionHandle,
    steps: &Value,
    error_summary: &str,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let summary = truncate_summary(error_summary);
    let n = db_execute!(
        pool,
        "UPDATE iam_bootstrap_init_requests \
         SET steps = ?, status = 'failed', last_error = ?, lease_expires_at = NULL, updated_at = ? \
         WHERE id = ? AND execution_owner = ? AND execution_epoch = ? \
           AND status IN ('running', 'selected', 'failed')",
        steps,
        summary,
        now,
        handle.request_id,
        &handle.owner,
        handle.epoch
    )?;
    Ok(n == 1)
}

/// 全部步骤完成 → 总状态 completed（唯一完成入口；调用方必须先用
/// `StepsState::all_completed()` 证实全部完成）。
pub async fn mark_completed(
    pool: &DbPool,
    handle: &ExecutionHandle,
    steps: &Value,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let n = db_execute!(
        pool,
        "UPDATE iam_bootstrap_init_requests \
         SET steps = ?, status = 'completed', finished_at = ?, lease_expires_at = NULL, updated_at = ? \
         WHERE id = ? AND execution_owner = ? AND execution_epoch = ? \
           AND status IN ('running', 'selected', 'failed')",
        steps,
        now,
        now,
        handle.request_id,
        &handle.owner,
        handle.epoch
    )?;
    Ok(n == 1)
}

/// last_error 列截断（列宽 255；摘要只含错误码与步骤 key，安全）。
fn truncate_summary(s: &str) -> String {
    s.chars().take(200).collect()
}

/// 本 IAM 实例标识（execution_owner 取值；hostname + pid，仅用于运维观测）。
pub fn instance_id() -> String {
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "iam".to_string());
    format!("{hostname}#{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_id_format() {
        let id = instance_id();
        assert!(!id.is_empty());
        assert!(id.contains('#'));
    }

    #[test]
    fn truncate_summary_bounds() {
        let long = "x".repeat(500);
        assert_eq!(truncate_summary(&long).chars().count(), 200);
    }
}
