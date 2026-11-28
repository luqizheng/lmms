//! Bootstrap 执行权：数据库原子 claim + owner/epoch fencing + 租约。
//!
//! 设计基线 Q5：请求仲裁（gate 选胜者）与执行权是两个机制。执行权通过
//! 数据库原子条件更新取得，`execution_epoch` 每次成功 claim 递增，作为
//! fencing token——所有步骤状态写入都必须携带 owner+epoch 条件，旧执行
//! 者在执行权被重新授予后无法继续写入。
//!
//! 不使用进程本地锁 / Mutex：跨实例互斥只由数据库保证。

use chrono::{DateTime, Duration, Utc};
use digua_common_db::{db_execute, db_fetch_optional, DbPool};
use serde_json::Value;
use sqlx::Error;

use super::gate::db_now;

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
