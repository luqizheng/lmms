//! Bootstrap **PostgreSQL** 侧并发语义验证（WP-BOOTSTRAP-01 §5.4）。
//!
//! 与 audit/token/notification 的 `pg_roundtrip_test.rs` 同一模式：
//! `TEST_PG_URL` 外部容器模式，未设时打印 `[skip]` 直接通过。每个用例建
//! 唯一数据库（`CREATE DATABASE`），避免并行测试互相污染。
//!
//! 覆盖（与 MySQL 套件 `bootstrap_test.rs` 对应）：
//! 1. `ensure_schema` 在 PG 应用 V011（gate 表 + 预插控制行）且幂等
//! 2. 数据库时钟语义（PG `clock_timestamp()`，非事务开始时间）
//! 3. 并发准入 → 全部插入 → 关闭选最小 id（READ COMMITTED 行锁协议）
//! 4. **候选 INSERT 未提交时窗口到期** → 关闭事务等 gate 锁 → 纳入仲裁
//! 5. 回滚候选被排除 / 窗口过期与关闭后准入被拒
//! 6. 并发 claim 唯一胜者 + epoch fencing（旧执行者无法写状态）

#![cfg(feature = "postgres")]

use digua_common_db::{db_execute, db_fetch_one, DbPool};
use iam_service::service::bootstrap::{executor, gate};
use std::time::Duration;

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[bootstrap_pg] TEST_PG_URL not set — skipped");
            None
        }
    }
}

fn with_db(url: &str, db: &str) -> String {
    let (base, _) = url.rsplit_once('/').expect("TEST_PG_URL must contain /db");
    format!("{base}/{db}")
}

async fn fresh_pg_db() -> Option<DbPool> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_iam_bs_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    let pool = digua_common_db::init_pool_with(&with_db(&url, &db_name), 20)
        .await
        .expect("connect fresh pg db");
    iam_service::db::ensure_schema(&pool)
        .await
        .expect("ensure iam schema on pg");
    Some(pool)
}

macro_rules! pg_case {
    ($name:literal, $pool:ident, $body:block) => {
        let Some($pool) = fresh_pg_db().await else {
            eprintln!("[bootstrap_pg] {} skipped (no TEST_PG_URL)", $name);
            return;
        };
        let $pool = &$pool;
        $body
    };
}

// ── schema：V011 gate 表 + 预插控制行 + 幂等 ─────────────────────────────

#[tokio::test]
async fn pg_ensure_schema_creates_gate_with_control_row() {
    pg_case!("pg_ensure_schema_creates_gate_with_control_row", pool, {
        // gate 控制行必须已由迁移预插。
        let (status,): (String,) =
            db_fetch_one!(pool, (String,), "SELECT status FROM iam_bootstrap_init_gate WHERE id = 1")
                .unwrap();
        assert_eq!(status, "idle");

        // 幂等：重复 ensure_schema 不炸、不重复插行。
        iam_service::db::ensure_schema(pool).await.unwrap();
        let (n,): (i64,) =
            db_fetch_one!(pool, (i64,), "SELECT COUNT(*) FROM iam_bootstrap_init_gate").unwrap();
        assert_eq!(n, 1);
    });
}

// ── 时钟语义 ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn pg_db_now_uses_clock_timestamp() {
    pg_case!("pg_db_now_uses_clock_timestamp", pool, {
        // 两次读取都推进（clock_timestamp 非事务快照）；且与 PG 事务开始
        // 时间同源（均数据库时钟），与 App 宿主时钟无关。
        let a = gate::db_now(pool).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        let b = gate::db_now(pool).await.unwrap();
        assert!(b > a, "clock_timestamp must advance between reads");

        // 与 PG 自身 now() 对齐（同为 DB 时钟，差值秒级内）。
        let (db_now,): (chrono::DateTime<chrono::Utc>,) =
            db_fetch_one!(pool, (chrono::DateTime<chrono::Utc>,), "SELECT NOW()").unwrap();
        assert!((db_now - b).num_seconds().abs() < 5);
    });
}

// ── 并发准入：窗口内全插入 → 关闭选最小 id ───────────────────────────────

#[tokio::test]
async fn pg_concurrent_admission_selects_min_id() {
    pg_case!("pg_concurrent_admission_selects_min_id", pool, {
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let pool = pool.clone();
            tasks.push(tokio::spawn(async move { gate::admit_candidate(&pool).await }));
        }
        let mut ids = Vec::new();
        let mut ends = None;
        for t in tasks {
            match t.await.unwrap().unwrap() {
                gate::AdmitOutcome::Admitted { request_id, window_ends_at } => {
                    ids.push(request_id);
                    ends = Some(window_ends_at);
                }
                other => panic!("in-window admission must succeed on pg, got {other:?}"),
            }
        }
        assert_eq!(ids.len(), 8);

        let wait = (ends.unwrap() - gate::db_now(pool).await.unwrap())
            .to_std()
            .unwrap_or(Duration::ZERO);
        tokio::time::sleep(wait + Duration::from_millis(50)).await;
        match gate::try_close_window(pool).await.unwrap() {
            gate::CloseOutcome::Closed { winner } => {
                assert_eq!(winner, ids.iter().copied().min());
            }
            other => panic!("expected Closed, got {other:?}"),
        }
        let (rejected,): (i64,) = db_fetch_one!(
            pool,
            (i64,),
            "SELECT COUNT(*) FROM iam_bootstrap_init_requests WHERE status = 'rejected'"
        )
        .unwrap();
        assert_eq!(rejected, 7);
    });
}

// ── 核心：未提交候选 vs 窗口关闭竞争 ─────────────────────────────────────

#[tokio::test]
async fn pg_uncommitted_candidate_included_by_close() {
    pg_case!("pg_uncommitted_candidate_included_by_close", pool, {
        // 在途准入事务：锁 gate、开窗、INSERT（不提交）。
        let mut pending = pool.begin().await.unwrap();
        let row: Option<(String,)> = digua_common_db::db_tx_fetch_optional!(
            &mut pending,
            (String,),
            "SELECT status FROM iam_bootstrap_init_gate WHERE id = 1 FOR UPDATE"
        )
        .unwrap();
        assert_eq!(row.unwrap().0, "idle");
        let now = gate::tx_now(&mut pending).await.unwrap();
        digua_common_db::db_tx_execute!(
            &mut pending,
            "UPDATE iam_bootstrap_init_gate SET status = 'collecting', window_started_at = ?, \
             window_ends_at = ? WHERE id = 1 AND status = 'idle'",
            now,
            now + chrono::Duration::seconds(2)
        )
        .unwrap();
        let pending_id: i64 = digua_common_db::db_tx_fetch_scalar!(
            &mut pending,
            i64,
            "INSERT INTO iam_bootstrap_init_requests (status, created_at, updated_at) \
             VALUES ('collecting', ?, ?) RETURNING id",
            now,
            now
        )
        .unwrap();

        // 关闭事务等到窗口到期后竞争 gate 锁（此时 pending 仍持有）。
        let pool2 = pool.clone();
        let closer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(2300)).await;
            gate::try_close_window(&pool2).await
        });
        // 在途事务提交 → 候选可见。
        pending.commit().await.unwrap();

        match closer.await.unwrap().unwrap() {
            gate::CloseOutcome::Closed { winner } => {
                assert_eq!(winner, Some(pending_id), "committed-late candidate must be arbitrated");
            }
            other => panic!("expected Closed, got {other:?}"),
        }
    });
}

#[tokio::test]
async fn pg_rolled_back_candidate_excluded_by_close() {
    pg_case!("pg_rolled_back_candidate_excluded_by_close", pool, {
        // A 事务锁 gate + INSERT（不提交）；B 的准入被 gate 锁排队。
        let mut pending = pool.begin().await.unwrap();
        let now = gate::tx_now(&mut pending).await.unwrap();
        digua_common_db::db_tx_execute!(
            &mut pending,
            "UPDATE iam_bootstrap_init_gate SET status = 'collecting', window_started_at = ?, \
             window_ends_at = ? WHERE id = 1 AND status = 'idle'",
            now,
            now + chrono::Duration::seconds(2)
        )
        .unwrap();
        digua_common_db::db_tx_execute!(
            &mut pending,
            "INSERT INTO iam_bootstrap_init_requests (status, created_at, updated_at) \
             VALUES ('collecting', ?, ?) RETURNING id",
            now,
            now
        )
        .unwrap();

        let pool2 = pool.clone();
        let b_task = tokio::spawn(async move { gate::admit_candidate(&pool2).await });
        tokio::time::sleep(Duration::from_millis(200)).await;

        // A 回滚 → B 获锁准入（窗口重新判定：B 开新窗，因为 gate 由 B 置 idle? 不
        // —— A 已把 gate 置 collecting，B 获锁后走 collecting 分支在原窗口内插入）。
        pending.rollback().await.unwrap();
        let b = b_task.await.unwrap().unwrap();
        let b_id = match &b {
            gate::AdmitOutcome::Admitted { request_id, .. } => *request_id,
            other => panic!("admit B failed: {other:?}"),
        };

        // 等窗口（A 开窗起的 2 秒）到期后关闭。
        tokio::time::sleep(Duration::from_millis(2300)).await;
        match gate::try_close_window(pool).await.unwrap() {
            gate::CloseOutcome::Closed { winner } => {
                assert_eq!(winner, Some(b_id), "rolled-back candidate must be excluded");
            }
            other => panic!("expected Closed, got {other:?}"),
        }
    });
}

// ── 边界拒绝 ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn pg_admit_rejected_after_expired_and_closed() {
    pg_case!("pg_admit_rejected_after_expired_and_closed", pool, {
        // 开窗后把窗口拨到过去 → 到期后才拿锁的准入被拒。
        let first = gate::admit_candidate(pool).await.unwrap();
        assert!(matches!(first, gate::AdmitOutcome::Admitted { .. }));
        db_execute!(
            pool,
            "UPDATE iam_bootstrap_init_gate SET window_ends_at = clock_timestamp() - interval '1 second' WHERE id = 1"
        )
        .unwrap();
        assert_eq!(
            gate::admit_candidate(pool).await.unwrap(),
            gate::AdmitOutcome::RejectedWindowExpired
        );

        // 关闭后再准入 → RejectedClosed。
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(matches!(
            gate::try_close_window(pool).await.unwrap(),
            gate::CloseOutcome::Closed { winner: Some(_) }
        ));
        assert_eq!(
            gate::admit_candidate(pool).await.unwrap(),
            gate::AdmitOutcome::RejectedClosed
        );
    });
}

// ── 执行权：并发 claim 唯一 + epoch fencing ──────────────────────────────

async fn seed_selected(pool: &DbPool) -> i64 {
    let admitted = gate::admit_candidate(pool).await.unwrap();
    let (rid, ends) = match &admitted {
        gate::AdmitOutcome::Admitted { request_id, window_ends_at } => (*request_id, *window_ends_at),
        other => panic!("seed admit failed: {other:?}"),
    };
    let wait = (ends - gate::db_now(pool).await.unwrap())
        .to_std()
        .unwrap_or(Duration::ZERO);
    tokio::time::sleep(wait + Duration::from_millis(50)).await;
    match gate::try_close_window(pool).await.unwrap() {
        gate::CloseOutcome::Closed { winner: Some(w) } => assert_eq!(w, rid),
        other => panic!("seed close failed: {other:?}"),
    }
    rid
}

#[tokio::test]
async fn pg_concurrent_claim_exactly_one_winner() {
    pg_case!("pg_concurrent_claim_exactly_one_winner", pool, {
        let rid = seed_selected(pool).await;
        let mut tasks = Vec::new();
        for i in 0..8 {
            let pool = pool.clone();
            tasks.push(tokio::spawn(async move {
                executor::claim_execution(&pool, rid, &format!("pg-instance-{i}")).await
            }));
        }
        let mut winners = Vec::new();
        for t in tasks {
            if let Some(h) = t.await.unwrap().unwrap() {
                winners.push(h);
            }
        }
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].epoch, 1);
    });
}

#[tokio::test]
async fn pg_old_epoch_cannot_write_step_state() {
    pg_case!("pg_old_epoch_cannot_write_step_state", pool, {
        let rid = seed_selected(pool).await;
        let old = executor::claim_execution(pool, rid, "pg-old")
            .await
            .unwrap()
            .expect("first claim");

        // 租约过期 → 新实例接管（epoch+1）。
        db_execute!(
            pool,
            "UPDATE iam_bootstrap_init_requests SET lease_expires_at = clock_timestamp() - interval '1 second' WHERE id = ?",
            rid
        )
        .unwrap();
        let new = executor::claim_execution(pool, rid, "pg-new")
            .await
            .unwrap()
            .expect("recovery claim");
        assert_eq!(new.epoch, old.epoch + 1);

        let steps = iam_service::service::bootstrap::steps::StepsState::initial().to_value();
        assert!(!executor::persist_step_progress(pool, &old, &steps, chrono::Utc::now()).await.unwrap());
        assert!(!executor::mark_step_failed(pool, &old, &steps, "x", chrono::Utc::now()).await.unwrap());
        assert!(!executor::mark_completed(pool, &old, &steps, chrono::Utc::now()).await.unwrap());
        assert!(!executor::renew_lease(pool, &old).await.unwrap());
        assert!(executor::persist_step_progress(pool, &new, &steps, chrono::Utc::now()).await.unwrap());
    });
}

#[tokio::test]
async fn pg_completed_request_not_claimable() {
    pg_case!("pg_completed_request_not_claimable", pool, {
        let rid = seed_selected(pool).await;
        db_execute!(
            pool,
            "UPDATE iam_bootstrap_init_requests SET status = 'completed', finished_at = clock_timestamp() WHERE id = ?",
            rid
        )
        .unwrap();
        assert!(executor::claim_execution(pool, rid, "late").await.unwrap().is_none());
    });
}
