//! audit-service 在 **PostgreSQL** 上的读写往返验证。
//!
//! 与 config/file/token/prue 的 `pg_roundtrip_test.rs` 同一模式：
//! `TEST_PG_URL` 外部容器模式，未设时打印 `[skip]` 直接通过。
//! 每个用例建唯一数据库（`CREATE DATABASE`），避免并行测试互相污染。
//!
//! audit 没有 HTTP 写入入口（事件全部来自 MQ），写路径的生产代码就是
//! `consumer::persist`——按其文档注释的意图，直接调它来验证：
//! 1. `ON CONFLICT (event_id) DO UPDATE` 幂等（同一事件重复投递不产生第二行）
//! 2. `?::json` cast 写入的 payload 中文/嵌套结构原样往返
//! 3. `TIMESTAMPTZ(3) → DateTime<Utc>` 精确解码

#![cfg(feature = "postgres")]

use audit_service::consumer;
use digua_common_db::DbPool;
use digua_common_event::Envelope;
use serde_json::json;

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[audit_pg_roundtrip] TEST_PG_URL not set — skipped");
            None
        }
    }
}

/// 从 `postgres://u:p@h:p/dbname` 里替换库名。
fn with_db(url: &str, db: &str) -> String {
    let (base, _old) = url
        .rsplit_once('/')
        .expect("TEST_PG_URL must contain a /db");
    format!("{base}/{db}")
}

/// 建一个唯一数据库并返回连到它的 `DbPool`（已跑完 `ensure_schema`）。
async fn fresh_pg_pool() -> Option<DbPool> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_audit_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );

    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    let pool = audit_service::db::init_pool(&with_db(&url, &db_name))
        .await
        .expect("init_pool on fresh pg db");
    audit_service::db::ensure_schema(&pool)
        .await
        .expect("ensure_schema on pg");
    Some(pool)
}

/// 构造 MQ 投递形态的事件信封（字段与 consumer::persist 的 bind 顺序一一对应）。
///
/// `occurred_at` 截断到毫秒：`audit_events.occurred_at` 是 `TIMESTAMPTZ(3)`，
/// PG 会四舍五入亚毫秒部分，毫秒截断是唯一能精确断言往返的形态。
fn envelope(event_id: uuid::Uuid, payload: serde_json::Value) -> Envelope<serde_json::Value> {
    let now_millis = chrono::Utc::now().timestamp_millis();
    Envelope {
        event_id,
        event_type: "iam.user.updated".to_string(),
        schema_version: 1,
        occurred_at: chrono::DateTime::from_timestamp_millis(now_millis)
            .expect("millis timestamp is valid"),
        trace_id: Some("trace-中文-混排".to_string()),
        tenant_id: Some("7".to_string()),
        producer: "iam-service".to_string(),
        payload,
    }
}

/// `ensure_schema` 必须**幂等**：重复调用不得报错（marker 探测在 PG 走 current_schema()）。
#[tokio::test]
async fn pg_ensure_schema_is_idempotent() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    audit_service::db::ensure_schema(&pool)
        .await
        .expect("second ensure_schema must be a no-op");
    audit_service::db::ensure_schema(&pool)
        .await
        .expect("third ensure_schema must be a no-op");
}

/// persist 同一 event_id 两次：仍只有一行，payload 保持首次值
/// （`ON CONFLICT DO UPDATE` 只刷新 received_at），中文/嵌套 JSON 原样往返。
#[tokio::test]
async fn pg_persist_is_idempotent_and_json_roundtrips() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let payload = json!({
        "user": "张三",
        "action": "修改配置",
        "nested": { "ok": true, "count": 3 }
    });
    let env = envelope(uuid::Uuid::new_v4(), payload.clone());

    consumer::persist(&pool, &env)
        .await
        .expect("first persist must succeed on PG");
    // 第二次投递同一事件：ON CONFLICT 吸收重复键，必须 Ok 且不产生第二行。
    consumer::persist(&pool, &env)
        .await
        .expect("duplicate persist must be absorbed by ON CONFLICT");

    let pg = pool.as_postgres().expect("pg pool");
    let (count, stored): (i64, String) = sqlx::query_as(
        "SELECT COUNT(*) OVER (), payload_json::text FROM audit_events WHERE event_id = $1",
    )
    .bind(env.event_id.to_string())
    .fetch_one(pg)
    .await
    .expect("read back persisted event");
    assert_eq!(count, 1, "重复投递不得产生第二行");
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("payload_json::text must be valid JSON");
    assert_eq!(stored, payload, "payload JSON（含中文/嵌套）必须原样往返");

    // TIMESTAMPTZ(3) → DateTime<Utc> 精确解码（毫秒截断后相等）。
    let row: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT occurred_at, received_at FROM audit_events WHERE event_id = $1",
    )
    .bind(env.event_id.to_string())
    .fetch_one(pg)
    .await
    .expect("read back timestamps");
    assert_eq!(
        row.0.timestamp_millis(),
        env.occurred_at.timestamp_millis(),
        "occurred_at 必须按 TIMESTAMPTZ 精确往返"
    );
    assert!(row.1.timestamp() > 0, "received_at 必须有默认值写入");
}

/// V001 的 PG 侧列形态：IDENTITY 主键、BOOLEAN、JSON 列、唯一索引（幂等写的前提）。
#[tokio::test]
async fn pg_v001_column_shapes() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    let ty: (String,) = sqlx::query_as(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'audit_events' \
           AND column_name = 'occurred_at'",
    )
    .fetch_one(pg)
    .await
    .expect("describe occurred_at");
    assert_eq!(
        ty.0, "timestamp with time zone",
        "时间列必须是 TIMESTAMPTZ（DateTime<Utc> 唯一可解类型）"
    );

    let is_identity: (bool,) = sqlx::query_as(
        "SELECT is_identity = 'YES' FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'audit_login_logs' \
           AND column_name = 'id'",
    )
    .fetch_one(pg)
    .await
    .expect("describe login_logs.id");
    assert!(is_identity.0, "主键必须是 IDENTITY（AUTO_INCREMENT 的 PG 对应物）");

    let success_ty: (String,) = sqlx::query_as(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'audit_login_logs' \
           AND column_name = 'success'",
    )
    .fetch_one(pg)
    .await
    .expect("describe success");
    assert_eq!(success_ty.0, "boolean", "TINYINT(1) 必须映射为 BOOLEAN");
}
