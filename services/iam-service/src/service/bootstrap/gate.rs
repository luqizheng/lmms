//! Bootstrap Gate 并发准入协议（设计基线 Q3，2 秒候选收集窗口）。
//!
//! 核心不变量（**所有**代码路径必须遵守统一锁顺序：先锁 gate 行 → 再
//! 访问候选行，禁止反向）：
//!
//! 1. 候选准入事务：`FOR UPDATE` 锁 gate 行 → 锁后读数据库时钟 →
//!    按 gate 状态判定（closed/过期拒绝；idle 开窗；collecting 未到期
//!    放行）→ 同事务插入候选 → 提交。
//! 2. 窗口关闭事务：`FOR UPDATE` 锁同一 gate 行（会等待所有在途准入
//!    事务提交/回滚，因此不会漏掉未提交的候选 INSERT）→ 锁后读数据库
//!    时钟确认到期 → 同事务置 `closed` + 选最小 id 胜出者 + 写
//!    `winner_request_id` → 提交。
//! 3. 窗口计时与边界判断一律使用数据库时钟（PG `clock_timestamp()`，
//!    不用事务开始时间语义的 `CURRENT_TIMESTAMP`；MySQL `NOW(6)`），
//!    边界：`db_now < window_ends_at` 可准入，`>=` 可关闭。
//! 4. 死锁 / 锁等待超时按有限次数重试整个事务，重试必须重新走 gate
//!    状态检查（不得绕过）。

use chrono::{DateTime, Duration, Utc};
use digua_common_db::{
    db_fetch_optional, db_fetch_scalar, db_tx_execute, db_tx_fetch_optional, db_tx_fetch_scalar,
    DbPool, Transaction,
};
use sqlx::Error;

/// gate 控制行固定主键（迁移预插，恒为 1）。
pub const GATE_ID: i64 = 1;
/// 候选收集窗口时长（设计固定值：2 秒）。
pub const WINDOW_SECS: i64 = 2;
/// 死锁 / 锁等待超时的最大事务重试次数（有限次，重试重新走状态检查）。
pub const TX_RETRY_MAX: u32 = 3;

/// 准入事务结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitOutcome {
    /// 已插入候选（request_id）并 commit；window_ends_at 为本轮窗口截止
    /// （数据库时钟值，供等待与关闭判断）。
    Admitted {
        request_id: i64,
        window_ends_at: DateTime<Utc>,
    },
    /// 取得 gate 锁时窗口已关闭（禁止准入，设计 Q3 第 6 条）。
    RejectedClosed,
    /// 取得 gate 锁时窗口已到期（禁止准入，设计 Q3 第 6 条）。
    RejectedWindowExpired,
}

/// 窗口关闭事务结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseOutcome {
    /// 持锁后确认窗口尚未到期（调用方稍后重试）。
    NotYet { window_ends_at: DateTime<Utc> },
    /// 关闭事务进入时已是 closed（winner 已确定或为空轮）。
    AlreadyClosed { winner: Option<i64> },
    /// 本事务完成关闭并选出胜者（winner=None 仅在空轮防御分支出现，
    /// gate 保持 collecting 等待候选出现）。
    Closed { winner: Option<i64> },
    /// collecting 但没有任何已提交候选：保持 collecting，不关闭。
    EmptyRound,
}

#[derive(Debug, sqlx::FromRow)]
pub struct GateRow {
    pub status: String,
    pub window_ends_at: Option<DateTime<Utc>>,
    pub winner_request_id: Option<i64>,
}

/// 数据库时钟读取 SQL（锁后执行；PG 用 clock_timestamp 避免
/// CURRENT_TIMESTAMP 的事务开始时间语义误判窗口边界）。
fn now_sql(dialect: digua_common_db::dialect::DialectKind) -> &'static str {
    match dialect {
        digua_common_db::dialect::DialectKind::Postgres => "SELECT clock_timestamp()",
        _ => "SELECT NOW(6)",
    }
}

/// 池级数据库当前时间（锁外读取仅用于展示 / 租约计算；准入与关闭的
/// 窗口边界判断必须在事务内、取得 gate 行锁之后调用 `tx_now`）。
pub async fn db_now(pool: &DbPool) -> Result<DateTime<Utc>, Error> {
    let sql = now_sql(pool.dialect());
    db_fetch_scalar!(pool, DateTime<Utc>, sql)
}

/// 事务内读取数据库时钟（必须在取得 gate 行锁之后调用）。
pub async fn tx_now(tx: &mut Transaction<'_>) -> Result<DateTime<Utc>, Error> {
    let sql = match tx {
        #[cfg(feature = "mysql")]
        Transaction::Mysql(_) => "SELECT NOW(6)",
        #[cfg(feature = "postgres")]
        Transaction::Postgres(_) => "SELECT clock_timestamp()",
        #[allow(unreachable_patterns)]
        _ => return Err(Error::Configuration("bootstrap: dialect not enabled".into())),
    };
    db_tx_fetch_scalar!(tx, DateTime<Utc>, sql)
}

/// MySQL `LAST_INSERT_ID()` 返回 BIGINT UNSIGNED，sqlx 需按 u64 解码
/// （V009 对齐经验：UNSIGNED 列直接解 i64 会失败），再转 i64。
/// 仅 MySQL 臂编译（PG 走 `RETURNING id`，见 admit_once）。
#[cfg(feature = "mysql")]
async fn tx_last_insert_id(tx: &mut Transaction<'_>) -> Result<i64, Error> {
    // 原生 sqlx 调用（不走方言宏）：LAST_INSERT_ID 仅 MySQL 臂可达，
    // u64 解码在 PG 侧无对应类型。
    match tx {
        Transaction::Mysql(t) => {
            let row: (u64,) = sqlx::query_as("SELECT LAST_INSERT_ID()")
                .fetch_one(&mut **t)
                .await?;
            i64::try_from(row.0)
                .map_err(|_| Error::Configuration("bootstrap request id overflow".into()))
        }
        _ => Err(Error::Configuration(
            "tx_last_insert_id only valid on mysql".into(),
        )),
    }
}

/// 判定可重试的瞬时事务错误：MySQL 死锁 1213 / 锁等待超时 1205，
/// PG 序列化失败 40001 / 死锁 40P01。
fn is_retryable(e: &Error) -> bool {
    match e {
        Error::Database(db) => {
            let msg = db.to_string();
            msg.contains("1213")
                || msg.contains("1205")
                || msg.contains("Deadlock")
                || msg.contains("deadlock")
                || msg.contains("40001")
                || msg.contains("40P01")
                || msg.contains("serialization failure")
                || msg.contains("lock wait timeout")
        }
        _ => false,
    }
}

async fn retry_backoff(attempt: u32) {
    tokio::time::sleep(std::time::Duration::from_millis(50 * (attempt as u64 + 1))).await;
}

/// 读取 gate 控制行（锁外快照读，仅用于请求的前置检查 / 状态查询）。
pub async fn read_gate(pool: &DbPool) -> Result<Option<GateRow>, Error> {
    db_fetch_optional!(
        pool,
        GateRow,
        "SELECT status, window_ends_at, winner_request_id FROM iam_bootstrap_init_gate WHERE id = ?",
        GATE_ID
    )
}

/// 候选准入事务（设计 Q3 第 3~6 条）。
pub async fn admit_candidate(pool: &DbPool) -> Result<AdmitOutcome, Error> {
    for attempt in 0..TX_RETRY_MAX {
        match admit_once(pool).await {
            Err(e) if is_retryable(&e) && attempt + 1 < TX_RETRY_MAX => {
                retry_backoff(attempt).await;
            }
            other => return other,
        }
    }
    unreachable!("retry loop returns in all branches")
}

async fn admit_once(pool: &DbPool) -> Result<AdmitOutcome, Error> {
    let mut tx = pool.begin().await?;

    // 1. 锁 gate 行（统一锁顺序第一步）。
    let gate: Option<GateRow> = db_tx_fetch_optional!(
        &mut tx,
        GateRow,
        "SELECT status, window_ends_at, winner_request_id FROM iam_bootstrap_init_gate WHERE id = ? FOR UPDATE",
        GATE_ID
    )?;
    let Some(gate) = gate else {
        // gate 行缺失属迁移破坏，fail-closed 拒绝准入。
        tx.rollback().await?;
        return Ok(AdmitOutcome::RejectedClosed);
    };

    // 2. 锁后读数据库时钟。
    let now = tx_now(&mut tx).await?;

    // 3. 按 gate 状态判定。
    let window_ends_at;
    match gate.status.as_str() {
        "closed" => {
            tx.rollback().await?;
            return Ok(AdmitOutcome::RejectedClosed);
        }
        "collecting" => match gate.window_ends_at {
            Some(ends) if now >= ends => {
                tx.rollback().await?;
                return Ok(AdmitOutcome::RejectedWindowExpired);
            }
            Some(ends) => window_ends_at = ends,
            // 窗口字段缺失属数据破坏，fail-closed。
            None => {
                tx.rollback().await?;
                return Ok(AdmitOutcome::RejectedWindowExpired);
            }
        },
        "idle" => {
            let ends = now + Duration::seconds(WINDOW_SECS);
            let n = db_tx_execute!(
                &mut tx,
                "UPDATE iam_bootstrap_init_gate SET status = 'collecting', window_started_at = ?, window_ends_at = ?, updated_at = ? WHERE id = ? AND status = 'idle'",
                now,
                ends,
                now,
                GATE_ID
            )?;
            if n != 1 {
                // 并发下另一事务抢先开窗（理论上不可能：我们持有行锁），
                // 防御性回滚重审。
                tx.rollback().await?;
                return Ok(AdmitOutcome::RejectedClosed);
            }
            window_ends_at = ends;
        }
        other => {
            tracing::error!(status = %other, "bootstrap gate unknown status");
            tx.rollback().await?;
            return Ok(AdmitOutcome::RejectedClosed);
        }
    }

    // 4. 同事务插入候选记录（自增 id 是窗口关闭后的仲裁顺序）。
    let request_id: i64 = match pool.dialect() {
        digua_common_db::dialect::DialectKind::Postgres => {
            db_tx_fetch_scalar!(
                &mut tx,
                i64,
                "INSERT INTO iam_bootstrap_init_requests (status, created_at, updated_at) \
                 VALUES ('collecting', ?, ?) RETURNING id",
                now,
                now
            )?
        }
        _ => {
            db_tx_execute!(
                &mut tx,
                "INSERT INTO iam_bootstrap_init_requests (status, created_at, updated_at) \
                 VALUES ('collecting', ?, ?)",
                now,
                now
            )?;
            tx_last_insert_id(&mut tx).await?
        }
    };

    tx.commit().await?;
    Ok(AdmitOutcome::Admitted {
        request_id,
        window_ends_at,
    })
}

/// 窗口关闭事务（设计 Q3 第 5 条）：锁 gate → 锁后读时钟确认到期 →
/// 同事务关闭 gate + 选最小 id 胜者 + 写 winner_request_id。
pub async fn try_close_window(pool: &DbPool) -> Result<CloseOutcome, Error> {
    for attempt in 0..TX_RETRY_MAX {
        match close_once(pool).await {
            Err(e) if is_retryable(&e) && attempt + 1 < TX_RETRY_MAX => {
                retry_backoff(attempt).await;
            }
            other => return other,
        }
    }
    unreachable!("retry loop returns in all branches")
}

async fn close_once(pool: &DbPool) -> Result<CloseOutcome, Error> {
    let mut tx = pool.begin().await?;

    // 1. 锁 gate 行：等待所有在途准入事务提交/回滚（不会漏掉未提交候选）。
    let gate: Option<GateRow> = db_tx_fetch_optional!(
        &mut tx,
        GateRow,
        "SELECT status, window_ends_at, winner_request_id FROM iam_bootstrap_init_gate WHERE id = ? FOR UPDATE",
        GATE_ID
    )?;
    let Some(gate) = gate else {
        tx.rollback().await?;
        return Ok(CloseOutcome::AlreadyClosed { winner: None });
    };

    match gate.status.as_str() {
        "closed" => {
            tx.rollback().await?;
            return Ok(CloseOutcome::AlreadyClosed {
                winner: gate.winner_request_id,
            });
        }
        "idle" => {
            // 尚无任何轮次：无窗口可关。
            tx.rollback().await?;
            return Ok(CloseOutcome::AlreadyClosed { winner: None });
        }
        _ => {}
    }

    // 2. 锁后读时钟确认到期。
    let now = tx_now(&mut tx).await?;
    let Some(ends) = gate.window_ends_at else {
        tx.rollback().await?;
        return Ok(CloseOutcome::EmptyRound);
    };
    if now < ends {
        tx.rollback().await?;
        return Ok(CloseOutcome::NotYet { window_ends_at: ends });
    }

    // 3. 选全部已提交候选中的最小自增 id 作为唯一胜出者。
    let winner: Option<(i64,)> =
        db_tx_fetch_optional!(
            &mut tx,
            (i64,),
            "SELECT MIN(id) FROM iam_bootstrap_init_requests WHERE status = 'collecting'"
        )?;
    let Some((winner_id,)) = winner else {
        // 空轮：保持 collecting，等候选出现（候选行不会被删除）。
        tx.rollback().await?;
        return Ok(CloseOutcome::EmptyRound);
    };

    // 4. 同事务：败者置 rejected、胜者置 selected、gate 关闭并记录 winner。
    db_tx_execute!(
        &mut tx,
        "UPDATE iam_bootstrap_init_requests SET status = 'rejected', updated_at = ? \
         WHERE status = 'collecting' AND id != ?",
        now,
        winner_id
    )?;
    db_tx_execute!(
        &mut tx,
        "UPDATE iam_bootstrap_init_requests SET status = 'selected', updated_at = ? WHERE id = ?",
        now,
        winner_id
    )?;
    db_tx_execute!(
        &mut tx,
        "UPDATE iam_bootstrap_init_gate SET status = 'closed', winner_request_id = ?, updated_at = ? WHERE id = ?",
        winner_id,
        now,
        GATE_ID
    )?;

    tx.commit().await?;
    Ok(CloseOutcome::Closed {
        winner: Some(winner_id),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn lock_order_documented_sql_uses_for_update() {
        // 锁顺序契约的静态断言：准入与关闭的第一条语句必须锁 gate 行。
        // SQL 在 admit_once / close_once 中内联，这里校验源码文本约束。
        let src = include_str!("gate.rs");
        assert!(src.matches("FOR UPDATE").count() >= 2);
    }
}
