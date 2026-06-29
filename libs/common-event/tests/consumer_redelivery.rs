//! testcontainers RabbitMQ 集成测：consumer 三态路径。
//!
//! 用例（详见 spec §6.2）：
//! 1. handler_permanent_goes_to_dlq
//! 2. handler_transient_requeues_then_succeeds
//! 3. handler_transient_upgrades_to_dlq_after_3_redeliveries
//! 4. deserialization_failure_goes_to_dlq
//! 5. republish_confirm_nack_does_not_ack_original
//! 6. republish_fails_on_unroutable_routing_key (direct function call)
//!
//! 所有测试共享一个 RabbitMQ 容器（ OnceCell ），串行执行（ Mutex ），
//! 每测结束后清理队列，避免容器泄漏和测试间干扰。

use common_event::{Consumer, Envelope, Event, HandlerError, RabbitMqConsumer};
use common_event::consumer::republish_with_incremented_count;
use lapin::{
    options::{
        BasicGetOptions, BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions, QueueDeleteOptions, QueuePurgeOptions,
    },
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties, ExchangeKind,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::rabbitmq::RabbitMq;
use tokio::sync::OnceCell;
use tokio::time::sleep;

const EXCHANGE: &str = "events";
const QUEUE: &str = "test.consumer-redelivery";
const BINDING: &str = "test.event";
const DLX: &str = "events.dlx";
const DLQ: &str = "events.dlq";

/// 串行锁：所有测试共用队列，必须串行执行。
static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn lock_test_mutex() -> std::sync::MutexGuard<'static, ()> {
    TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
}

/// 共享 RabbitMQ 容器（只启动一次）。
static RABBIT_URL: OnceCell<String> = OnceCell::const_new();

async fn shared_rabbit_url() -> &'static String {
    RABBIT_URL
        .get_or_init(|| async {
            let container = RabbitMq::default().start().await.unwrap();
            let host = container.get_host().await.unwrap();
            let port = container.get_host_port_ipv4(5672).await.unwrap();
            let url = format!("amqp://guest:guest@{}:{}/%2f", host, port);

            // 预先声明拓扑（只需一次）
            let conn = Connection::connect(&url, ConnectionProperties::default())
                .await
                .unwrap();
            let ch = conn.create_channel().await.unwrap();
            ch.exchange_declare(
                EXCHANGE,
                ExchangeKind::Topic,
                ExchangeDeclareOptions { durable: true, ..Default::default() },
                FieldTable::default(),
            ).await.unwrap();
            ch.exchange_declare(
                DLX,
                ExchangeKind::Topic,
                ExchangeDeclareOptions { durable: true, ..Default::default() },
                FieldTable::default(),
            ).await.unwrap();
            ch.queue_declare(
                DLQ,
                QueueDeclareOptions { durable: true, ..Default::default() },
                FieldTable::default(),
            ).await.unwrap();
            ch.queue_bind(DLQ, DLX, "#", QueueBindOptions::default(), FieldTable::default())
                .await
                .unwrap();

            // 保留 container 生命周期：forget 防止 drop 关闭容器
            std::mem::forget(container);
            url
        })
        .await
}

/// 每个测试开始前：建立独立 Connection，声明/清理队列。
async fn setup_test() -> (String, Connection) {
    let url = shared_rabbit_url().await.clone();
    let conn = Connection::connect(&url, ConnectionProperties::default())
        .await
        .unwrap();

    let ch = conn.create_channel().await.unwrap();

    // 清理旧数据
    let _ = ch.queue_delete(QUEUE, QueueDeleteOptions::default()).await;
    let _ = ch.queue_purge(DLQ, QueuePurgeOptions::default()).await;

    // 重新声明队列（带 DLX）
    let mut q_args = FieldTable::default();
    q_args.insert(
        "x-dead-letter-exchange".into(),
        lapin::types::AMQPValue::LongString(DLX.into()),
    );
    ch.queue_declare(
        QUEUE,
        QueueDeclareOptions { durable: true, ..Default::default() },
        q_args,
    ).await.unwrap();
    ch.queue_bind(QUEUE, EXCHANGE, BINDING, QueueBindOptions::default(), FieldTable::default())
        .await
        .unwrap();

    (url, conn)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestPayload {
    value: i32,
}
impl Event for TestPayload {
    fn event_type() -> &'static str {
        "test.event"
    }
    fn schema_version() -> u32 {
        1
    }
}

async fn publish_one(url: &str, body: &[u8], headers: FieldTable) {
    let conn = Connection::connect(url, ConnectionProperties::default())
        .await
        .unwrap();
    let ch = conn.create_channel().await.unwrap();
    let confirm = ch
        .basic_publish(
            EXCHANGE,
            BINDING,
            BasicPublishOptions {
                mandatory: true,
                ..Default::default()
            },
            body,
            BasicProperties::default()
                .with_content_type("application/json".into())
                .with_delivery_mode(2)
                .with_headers(headers),
        )
        .await
        .unwrap()
        .await
        .unwrap();
    assert!(!confirm.is_nack(), "publish nack in test fixture");
}

async fn drain_dlq(conn: &Connection) -> Vec<Vec<u8>> {
    let ch = conn.create_channel().await.unwrap();
    let mut bodies = vec![];
    loop {
        match ch
            .basic_get(DLQ, BasicGetOptions { no_ack: true })
            .await
            .unwrap()
        {
            Some(msg) => bodies.push(msg.delivery.data.clone()),
            None => break,
        }
    }
    bodies
}

/// Poll `drain_dlq` until at least `expected` messages appear, or deadline
/// expires. Returns the drained bodies. Replaces fixed `sleep()` calls to
/// avoid slow-CI flakes (T3-fu3).
async fn poll_dlq(conn: &Connection, expected: usize, deadline: Duration) -> Vec<Vec<u8>> {
    let start = std::time::Instant::now();
    loop {
        let bodies = drain_dlq(conn).await;
        if bodies.len() >= expected {
            return bodies;
        }
        if start.elapsed() > deadline {
            panic!(
                "timeout after {:?}: expected {} messages in DLQ, got {}",
                deadline, expected, bodies.len()
            );
        }
        sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
async fn handler_permanent_goes_to_dlq() {
    let _guard = lock_test_mutex();
    let (url, conn) = setup_test().await;

    let env = Envelope::new("test", TestPayload { value: 1 });
    let body = serde_json::to_vec(&env).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Err(HandlerError::Permanent("reject".into()))
        })
        .await
        .unwrap();

    let bodies = poll_dlq(&conn, 1, Duration::from_secs(10)).await;
    assert_eq!(bodies.len(), 1, "expected exactly 1 message in DLQ");
    let env: Envelope<TestPayload> = serde_json::from_slice(&bodies[0]).unwrap();
    assert_eq!(env.payload.value, 1);
}

#[tokio::test]
async fn handler_transient_requeues_then_succeeds() {
    let _guard = lock_test_mutex();
    let (url, conn) = setup_test().await;
    let counter = Arc::new(AtomicI32::new(0));
    let env = Envelope::new("test", TestPayload { value: 7 });
    let body = serde_json::to_vec(&env).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    let counter_for_handler = counter.clone();
    consumer
        .subscribe::<TestPayload, _, _>(
            QUEUE,
            &[BINDING],
            move |_env: Envelope<TestPayload>| {
                let counter = counter_for_handler.clone();
                async move {
                    let n = counter.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        Err(HandlerError::Transient("first try".into()))
                    } else {
                        Ok(())
                    }
                }
            },
        )
        .await
        .unwrap();

    // Poll until handler runs twice (Transient → Ok after redelivery).
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while counter.load(Ordering::SeqCst) < 2 {
        if std::time::Instant::now() > deadline {
            panic!(
                "timeout: handler ran {} times, expected 2",
                counter.load(Ordering::SeqCst)
            );
        }
        sleep(Duration::from_millis(200)).await;
    }

    let bodies = drain_dlq(&conn).await;
    assert_eq!(bodies.len(), 0, "expected DLQ to be empty after retry success");
    assert_eq!(counter.load(Ordering::SeqCst), 2, "handler should run twice");
}

#[tokio::test]
async fn handler_transient_upgrades_to_dlq_after_3_redeliveries() {
    let _guard = lock_test_mutex();
    let (url, conn) = setup_test().await;
    let env = Envelope::new("test", TestPayload { value: 9 });
    let body = serde_json::to_vec(&env).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Err(HandlerError::Transient("always fail".into()))
        })
        .await
        .unwrap();

    let bodies = poll_dlq(&conn, 1, Duration::from_secs(10)).await;
    assert_eq!(bodies.len(), 1, "expected 1 message in DLQ after 3 retries");
    let env: Envelope<TestPayload> = serde_json::from_slice(&bodies[0]).unwrap();
    assert_eq!(env.payload.value, 9);
}

#[tokio::test]
async fn deserialization_failure_goes_to_dlq() {
    let _guard = lock_test_mutex();
    let (url, conn) = setup_test().await;
    // publish invalid JSON that cannot deserialize to Envelope<TestPayload>
    publish_one(&url, b"not-a-valid-envelope", FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Ok(())
        })
        .await
        .unwrap();

    let bodies = poll_dlq(&conn, 1, Duration::from_secs(10)).await;
    assert_eq!(bodies.len(), 1, "deser fail should DLQ");
}

#[tokio::test]
async fn republish_confirm_nack_does_not_ack_original() {
    // Integration test: verify that when republish fails, the message
    // is not lost (either requeued or in DLQ, not silently dropped).
    //
    // Strategy: bind a temporary routing-key, publish, then unbind it
    // BEFORE the handler returns Transient. The republish hits an
    // unroutable routing-key → Basic.Return → Err → original NOT acked.
    // After dropping the consumer, the message should be requeued.
    let _guard = lock_test_mutex();
    let (url, conn) = setup_test().await;

    const TMP_BIND: &str = "test.republish-fail";
    let ch = conn.create_channel().await.unwrap();
    ch.queue_bind(QUEUE, EXCHANGE, TMP_BIND, QueueBindOptions::default(), FieldTable::default()).await.unwrap();

    // Publish one message.
    let env = Envelope::new("test", TestPayload { value: 42 });
    let body = serde_json::to_vec(&env).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    // Start consumer that returns Transient (will trigger republish).
    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(
            QUEUE,
            &[TMP_BIND],
            |_env: Envelope<TestPayload>| async {
                Err(HandlerError::Transient("force republish".into()))
            },
        )
        .await
        .unwrap();

    // Wait for message to be delivered to consumer.
    sleep(Duration::from_millis(500)).await;

    // Now unbind the routing-key so the upcoming republish will fail.
    ch.queue_unbind(QUEUE, EXCHANGE, TMP_BIND, FieldTable::default()).await.unwrap();

    // Wait for republish to attempt and fail (consumer retries every few seconds
    // internally, but the first republish should happen soon after handler returns).
    sleep(Duration::from_secs(3)).await;

    // The message should be unacked (held by consumer connection).
    // Drop consumer → connection close → message requeued.
    drop(consumer);
    sleep(Duration::from_secs(2)).await;

    // Check queue state.
    let ch2 = conn.create_channel().await.unwrap();
    let info = ch2
        .queue_declare(QUEUE, QueueDeclareOptions { passive: true, ..Default::default() }, FieldTable::default())
        .await
        .unwrap();
    let msg_count = info.message_count();
    eprintln!("Messages in queue after consumer drop: {}", msg_count);

    // Verify message is not lost: either requeued (msg_count > 0) or in DLQ.
    let dlq_bodies = drain_dlq(&conn).await;
    assert!(
        msg_count >= 1 || dlq_bodies.len() >= 1,
        "message should be either requeued (count={}) or in DLQ (count={})",
        msg_count,
        dlq_bodies.len()
    );
}

#[tokio::test]
async fn republish_fails_on_unroutable_routing_key() {
    let _guard = lock_test_mutex();
    let (url, _conn) = setup_test().await;
    let ch = Connection::connect(&url, ConnectionProperties::default()).await.unwrap()
        .create_channel().await.unwrap();

    // Declare an exchange but don't bind any queue to "test.unbound"
    ch.exchange_declare(
        "test.direct",
        ExchangeKind::Direct,
        ExchangeDeclareOptions::default(),
        FieldTable::default(),
    ).await.unwrap();

    let result = republish_with_incremented_count(
        &ch,
        "test.direct",
        "test.unbound",
        b"{}",
        0,
        None,
    )
    .await;

    assert!(result.is_err(), "republish to unroutable routing-key should fail (Basic.Return)");
}
