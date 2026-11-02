//! 端到端：testcontainers MySQL，构造 envelope.trace_id=Some("test-trace")，
//! 调真 MySqlOutboxRepo::enqueue，读回 envelope_json，断言 trace_id 字段正确入库。
//!
//! 覆盖 §6.1.C 集成测：fill_trace_id 与 &mut Envelope 串联起来后，caller 传入
//! 的 trace_id 确实写入 envelope_json JSON 列。
//!
//! 自动注入分支（fill_trace_id 在 current_trace_id 返回 None 时保持 None）
//! 已在 `enqueue_trace_id_test` 用纯函数覆盖；OTel 上下文中自动注入的
//! round-trip 留给 §6.2 e2e 跑全栈验证。

use digua_common_event::envelope::Envelope;
use digua_common_event::event::Event;
use digua_common_event::outbox::{AnyTx, MySqlOutboxRepo, OutboxRepo};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::mysql::Mysql;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestPayload;

impl Event for TestPayload {
    fn event_type() -> &'static str {
        "test.payload"
    }
    fn schema_version() -> u32 {
        1
    }
}

async fn setup_pool() -> (MySqlPool, ContainerAsync<Mysql>) {
    let container = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(3306).await.unwrap();
    let url = format!("mysql://root@{host}:{port}/test");
    let pool = MySqlPool::connect(&url).await.unwrap();

    sqlx::query(
        "CREATE TABLE iam_outbox_events (
            id BIGINT NOT NULL AUTO_INCREMENT,
            event_id CHAR(36) NOT NULL,
            event_type VARCHAR(128) NOT NULL,
            routing_key VARCHAR(128) NOT NULL,
            envelope_json JSON NOT NULL,
            created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
            published_at DATETIME(3) NULL,
            PRIMARY KEY (id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    (pool, container)
}

#[tokio::test]
async fn enqueue_writes_caller_supplied_trace_id_to_db() {
    let (pool, _container) = setup_pool().await;
    let repo = MySqlOutboxRepo::new(pool.clone());

    let mut env = Envelope::new("test", TestPayload).with_trace_id("test-trace-fixed");

    let mut tx = pool.begin().await.unwrap();
    repo.enqueue(&mut AnyTx::Mysql(&mut tx), &mut env).await.unwrap();
    tx.commit().await.unwrap();

    let row: (serde_json::Value,) =
        sqlx::query_as("SELECT envelope_json FROM iam_outbox_events ORDER BY id DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    let parsed = row.0;
    assert_eq!(
        parsed.get("trace_id").and_then(|v| v.as_str()),
        Some("test-trace-fixed"),
        "trace_id should round-trip through JSON column"
    );
}
