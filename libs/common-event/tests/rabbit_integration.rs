use common_event::consumer::{Consumer, RabbitMqConsumer};
use common_event::events::IamUserCreated;
use common_event::outbox::{MySqlOutboxRepo, OutboxRepo};
use common_event::publisher_loop::{OutboxPublisher, OutboxPublisherOptions};
use common_event::{Envelope, Publisher, RabbitMqPublisher};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::mysql::Mysql;
use testcontainers_modules::rabbitmq::RabbitMq;

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

async fn rabbit_url() -> (String, ContainerAsync<RabbitMq>) {
    let c = RabbitMq::default().start().await.unwrap();
    let host = c.get_host().await.unwrap();
    let port = c.get_host_port_ipv4(5672).await.unwrap();
    (format!("amqp://guest:guest@{}:{}/%2f", host, port), c)
}

async fn connect_publisher(url: &str) -> RabbitMqPublisher {
    let mut last = None;
    for _ in 0..30 {
        match RabbitMqPublisher::new(url, "events").await {
            Ok(p) => return p,
            Err(e) => {
                last = Some(e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!("rabbitmq connect failed: {:?}", last);
}

async fn make_mysql() -> (MySqlPool, ContainerAsync<Mysql>) {
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
async fn publish_sends_to_topic_exchange() {
    let (url, _rmq) = rabbit_url().await;
    let pub_ = connect_publisher(&url).await;
    let env = Envelope::new(
        "iam-service",
        IamUserCreated {
            user_id: "u1".to_string(),
            username: "alice".to_string(),
            email: None,
            org_unit_id: None,
        },
    );
    pub_.publish(&env).await.expect("publish ok");
}

#[tokio::test]
async fn outbox_loop_publishes_and_consumer_receives() {
    let (url, _rmq) = rabbit_url().await;
    let (pool, _mysql) = make_mysql().await;

    // Subscribe a consumer to iam.# BEFORE publishing so the binding exists.
    let received = Arc::new(AtomicUsize::new(0));
    let received_in_handler = received.clone();
    let consumer = RabbitMqConsumer::new(&url).await.expect("consumer connect");
    consumer
        .subscribe::<IamUserCreated, _, _>(
            "test.iam-events",
            &["iam.#"],
            move |env: Envelope<IamUserCreated>| {
                let counter = received_in_handler.clone();
                async move {
                    assert_eq!(env.payload.username, "alice");
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .expect("subscribe");

    // Enqueue an event into the outbox.
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

    // Run the outbox publisher once — it should publish the pending row.
    let publisher = connect_publisher(&url).await;
    let outbox = OutboxPublisher::new(
        MySqlOutboxRepo::with_table(pool.clone(), "outbox_events"),
        publisher,
        OutboxPublisherOptions {
            poll_interval: Duration::from_millis(200),
            batch_size: 10,
        },
    );
    let published = outbox.run_once().await.expect("run_once");
    assert_eq!(published, 1, "one pending row should be published");

    // Wait for the consumer to receive + ack.
    let mut got = false;
    for _ in 0..50 {
        if received.load(Ordering::SeqCst) >= 1 {
            got = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(got, "consumer did not receive the published event in time");

    // The published row should not be re-claimed.
    let repo2 = MySqlOutboxRepo::with_table(pool.clone(), "outbox_events");
    let remaining = repo2.claim_batch(10).await.unwrap();
    assert!(remaining.is_empty(), "published row must be marked, not re-claimed");
}
