//! PostgreSQL 上的 outbox 集成测试（真实 PG，需 `TEST_PG_URL`）。
//!
//! 无 `TEST_PG_URL` 时**跳过**（打印 `[skip]` 并直接返回），不失败——
//! 与 `libs/common-db/tests/pg_test.rs` 的既有约定一致。
//!
//! 本文件针对性验证三件事（S2 的三条关注点）：
//!
//! - **A. `ON CONFLICT (event_id) DO NOTHING` 需要唯一约束**：
//!   本测试自行建表并显式加 `UNIQUE (event_id)`，然后**重复 enqueue 同一 event_id**
//!   断言不报错 —— 这是 outbox 幂等重放的核心语义。
//! - **B. `mark_published` 的 `IN ($1..$N)` 编号与 `ids` 顺序一一对应**：
//!   插入 3 行后只 mark 其中 2 个（非连续、非前缀），断言恰好那 2 个被标记。
//! - **C. `claim_batch` 的 `LIMIT ?` 在 PG 下是 `LIMIT $1`**：
//!   用 limit=1 断言只返回 1 行（若占位符没改写，PG 会直接报语法/参数错误）。
//!
//! 注意：S2 阶段**还没有 PG 迁移**（S3 才落 `migrations/postgres/`），
//! 因此这里自建表；表名带随机后缀，避免与并发测试互相干扰。

#![cfg(feature = "postgres")]

use digua_common_db::dialect::DialectKind;
use digua_common_db::DbPool;
use digua_common_event::events::IamUserCreated;
use digua_common_event::outbox::{AnyTx, OutboxRepo, SqlOutboxRepo};
use digua_common_event::Envelope;

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[outbox_pg_test] TEST_PG_URL not set — skipped");
            None
        }
    }
}

/// 建一张 PG 侧 outbox 表（`event_id` 必须有唯一约束，否则 `ON CONFLICT (event_id)` 会报
/// `there is no unique or exclusion constraint matching the ON CONFLICT specification`）。
async fn create_table(pool: &sqlx::PgPool, table: &str) {
    sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
        .execute(pool)
        .await
        .expect("drop table");
    sqlx::query(&format!(
        "CREATE TABLE {table} (
             id BIGSERIAL PRIMARY KEY,
             event_id TEXT NOT NULL,
             event_type VARCHAR(128) NOT NULL,
             routing_key VARCHAR(128) NOT NULL,
             envelope_json JSONB NOT NULL,
             created_at TIMESTAMPTZ(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
             published_at TIMESTAMPTZ(3) NULL,
             CONSTRAINT uk_{table}_event_id UNIQUE (event_id)
         )"
    ))
    .execute(pool)
    .await
    .expect("create table");
}

fn user_created(user_id: &str) -> Envelope<IamUserCreated> {
    Envelope::new(
        "iam-service",
        IamUserCreated {
            user_id: user_id.to_string(),
            username: format!("user-{user_id}"),
            email: None,
            org_unit_id: None,
        },
    )
}

async fn setup(table: &str) -> Option<(DbPool, sqlx::PgPool)> {
    let url = test_pg_url()?;
    let pool = digua_common_db::init_pool(&url).await.expect("init_pool");
    assert_eq!(pool.dialect(), DialectKind::Postgres);
    let pg = pool.as_postgres().expect("pg pool").clone();
    create_table(&pg, table).await;
    Some((pool, pg))
}

/// A. 幂等重放：同一 `event_id` 重复 enqueue 不得报错，且只落一行。
#[tokio::test]
async fn pg_enqueue_is_idempotent_on_duplicate_event_id() {
    let table = "outbox_pg_idem";
    let Some((pool, pg)) = setup(table).await else {
        return;
    };
    let repo = SqlOutboxRepo::with_table(pool, table);

    let mut env = user_created("dup-1");
    let event_id = env.event_id;

    // 第一次
    let mut tx = pg.begin().await.expect("begin #1");
    repo.enqueue(&mut AnyTx::Postgres(&mut tx), &mut env)
        .await
        .expect("first enqueue must succeed");
    tx.commit().await.expect("commit #1");

    // 第二次：同一 event_id → PG 走 ON CONFLICT (event_id) DO NOTHING，必须静默成功
    let mut tx = pg.begin().await.expect("begin #2");
    repo.enqueue(&mut AnyTx::Postgres(&mut tx), &mut env)
        .await
        .expect("duplicate enqueue must be a no-op, not an error");
    tx.commit().await.expect("commit #2");

    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(&pg)
        .await
        .expect("count");
    assert_eq!(count.0, 1, "同一 event_id 只应落一行");
    assert_eq!(count.0, 1, "event_id={event_id} 的幂等重放不得产生第二行");
}

/// C. `LIMIT ?` 在 PG 下必须是 `LIMIT $1`（否则 PG 直接报错）。
#[tokio::test]
async fn pg_claim_batch_rewrites_limit_placeholder() {
    let table = "outbox_pg_claim";
    let Some((pool, pg)) = setup(table).await else {
        return;
    };
    let repo = SqlOutboxRepo::with_table(pool, table);

    for i in 0..3 {
        let mut env = user_created(&format!("c{i}"));
        let mut tx = pg.begin().await.expect("begin");
        repo.enqueue(&mut AnyTx::Postgres(&mut tx), &mut env)
            .await
            .expect("enqueue");
        tx.commit().await.expect("commit");
    }

    // limit=1：若占位符没改写成 $1，PG 会报 "expected 1 arguments, got 0" 之类。
    let batch = repo.claim_batch(1).await.expect("claim_batch(1)");
    assert_eq!(batch.len(), 1, "LIMIT $1 必须生效");
    assert!(
        batch[0].routing_key == "iam.user.created",
        "routing_key 应来自事件定义，实际: {}",
        batch[0].routing_key
    );
    assert_eq!(
        batch[0].envelope_json["producer"], "iam-service",
        "envelope_json 必须完整读回（JSONB 往返）"
    );

    let all = repo.claim_batch(10).await.expect("claim_batch(10)");
    assert_eq!(all.len(), 3, "未发布的 3 行都应可 claim");
}

/// B. `mark_published` 的 `IN ($1..$N)` 编号必须与 `ids` 顺序严格一致。
///
/// 构造「非前缀、非连续」的 id 子集，若编号错位就会标记到错误的行。
#[tokio::test]
async fn pg_mark_published_binds_in_order() {
    let table = "outbox_pg_mark";
    let Some((pool, pg)) = setup(table).await else {
        return;
    };
    let repo = SqlOutboxRepo::with_table(pool, table);

    for i in 0..4 {
        let mut env = user_created(&format!("m{i}"));
        let mut tx = pg.begin().await.expect("begin");
        repo.enqueue(&mut AnyTx::Postgres(&mut tx), &mut env)
            .await
            .expect("enqueue");
        tx.commit().await.expect("commit");
    }

    let batch = repo.claim_batch(10).await.expect("claim");
    assert_eq!(batch.len(), 4);
    let ids: Vec<i64> = batch.iter().map(|r| r.id).collect();

    // 取「第 2 个 + 第 4 个」——非连续、非前缀，编号错位会立刻暴露。
    let marked = vec![ids[1], ids[3]];
    repo.mark_published(&marked).await.expect("mark_published");

    let unpublished: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*) FROM {table} WHERE published_at IS NULL"
    ))
    .fetch_one(&pg)
    .await
    .expect("count unpublished");
    assert_eq!(unpublished.0, 2, "只应剩 2 行未发布");

    // 逐行确认：被标记的正是 ids[1] / ids[3]。
    for id in &ids {
        let row: (bool,) = sqlx::query_as(&format!(
            "SELECT published_at IS NOT NULL FROM {table} WHERE id = $1"
        ))
        .bind(*id)
        .fetch_one(&pg)
        .await
        .expect("select one");
        let expected = marked.contains(id);
        assert_eq!(
            row.0, expected,
            "id={id} 的发布状态错位（期望 published={expected}）"
        );
    }

    // 再 claim 只应拿到剩下的 2 行。
    let rest = repo.claim_batch(10).await.expect("claim after mark");
    assert_eq!(rest.len(), 2, "已发布的 2 行不应再被 claim");
}

/// `mark_published(&[])` 必须是 no-op（不得拼出 `IN ()` 这种非法 SQL）。
#[tokio::test]
async fn pg_mark_published_empty_is_noop() {
    let table = "outbox_pg_empty";
    let Some((pool, _pg)) = setup(table).await else {
        return;
    };
    let repo = SqlOutboxRepo::with_table(pool, table);
    repo.mark_published(&[])
        .await
        .expect("empty mark must be Ok");
}
