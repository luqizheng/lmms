use common_event::events::IamUserCreated;
use common_event::outbox::{MySqlOutboxRepo, OutboxRepo, OutboxRow};
use common_event::Envelope;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::mysql::Mysql;

const DDL: &str = r#"
CREATE TABLE outbox_events (
    id BIGINT NOT NULL AUTO_INCREMENT,
    event_id CHAR(36) NOT NULL,
    routing_key VARCHAR(128) NOT NULL,
    envelope_json JSON NOT NULL,
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    published_at DATETIME(3) NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_event_id (event_id),
    KEY idx_unpublished (published_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
"#;

async fn make_pool() -> (MySqlPool, ContainerAsync<Mysql>) {
    let c = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .unwrap();
    let host = c.get_host().await.unwrap();
    let port = c.get_host_port_ipv4(3306).await.unwrap();
    let url = format!("mysql://root@{}:{}/test", host, port);
    let pool = MySqlPoolOptions::new()
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await
        .unwrap();
    for stmt in DDL.split(';') {
        let s = stmt.trim();
        if !s.is_empty() {
            sqlx::query(s).execute(&pool).await.unwrap();
        }
    }
    (pool, c)
}

#[tokio::test]
async fn enqueue_then_claim_then_mark_published_round_trip() {
    let (pool, _container) = make_pool().await;
    let repo = MySqlOutboxRepo::with_table(pool.clone(), "outbox_events");

    let env = Envelope::new(
        "iam-service",
        IamUserCreated {
            user_id: "u1".to_string(),
            username: "alice".to_string(),
            email: None,
            org_unit_id: None,
        },
    );
    let mut tx = pool.begin().await.unwrap();
    repo.enqueue(&mut tx, &env).await.unwrap();
    tx.commit().await.unwrap();

    let batch = repo.claim_batch(10).await.unwrap();
    assert_eq!(batch.len(), 1);
    let row: &OutboxRow = &batch[0];
    assert_eq!(row.event_id, env.event_id);
    assert_eq!(row.routing_key, "iam.user.created");
    assert_eq!(row.envelope_json["producer"], "iam-service");

    repo.mark_published(&[row.id]).await.unwrap();
    let after = repo.claim_batch(10).await.unwrap();
    assert!(after.is_empty(), "published row should not be re-claimed");
}

#[tokio::test]
async fn enqueue_is_idempotent_on_duplicate_event_id() {
    let (pool, _container) = make_pool().await;
    let repo = MySqlOutboxRepo::with_table(pool.clone(), "outbox_events");

    let env = Envelope::new(
        "iam-service",
        IamUserCreated {
            user_id: "u1".to_string(),
            username: "alice".to_string(),
            email: None,
            org_unit_id: None,
        },
    );
    let mut tx = pool.begin().await.unwrap();
    repo.enqueue(&mut tx, &env).await.unwrap();
    tx.commit().await.unwrap();

    // Second enqueue with same event_id should be a no-op (UNIQUE key)
    let mut tx = pool.begin().await.unwrap();
    repo.enqueue(&mut tx, &env).await.unwrap();
    tx.commit().await.unwrap();

    let batch = repo.claim_batch(10).await.unwrap();
    assert_eq!(batch.len(), 1, "duplicate event_id must not enqueue twice");
}
