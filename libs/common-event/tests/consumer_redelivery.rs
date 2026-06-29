//! testcontainers RabbitMQ 集成测：consumer 三态路径。
//!
//! 用例（详见 spec §6.2）：
//! 1. handler_permanent_goes_to_dlq
//! 2. handler_transient_requeues_then_succeeds
//! 3. handler_transient_upgrades_to_dlq_after_3_redeliveries
//! 4. deserialization_failure_goes_to_dlq
//! 5. republish_confirm_nack_does_not_ack_original (todo! stub, requires broker nack simulation)

use common_event::{Consumer, Envelope, Event, HandlerError, RabbitMqConsumer};
use lapin::{
    options::{
        BasicGetOptions, BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties, ExchangeKind,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::rabbitmq::RabbitMq;
use tokio::time::sleep;

const EXCHANGE: &str = "events";
const QUEUE: &str = "test.consumer-redelivery";
const BINDING: &str = "test.event";
const DLX: &str = "events.dlx";
const DLQ: &str = "events.dlq";

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

async fn setup_rabbit() -> (String, Connection) {
    let container = RabbitMq::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5672).await.unwrap();
    let url = format!("amqp://guest:guest@{}:{}/%2f", host, port);
    let conn = Connection::connect(&url, ConnectionProperties::default())
        .await
        .unwrap();

    // pre-declare topology mirroring Phase 4 definitions.json
    let ch = conn.create_channel().await.unwrap();
    ch.exchange_declare(
        EXCHANGE,
        ExchangeKind::Topic,
        ExchangeDeclareOptions {
            durable: true,
            ..Default::default()
        },
        FieldTable::default(),
    )
    .await
    .unwrap();
    ch.exchange_declare(
        DLX,
        ExchangeKind::Topic,
        ExchangeDeclareOptions {
            durable: true,
            ..Default::default()
        },
        FieldTable::default(),
    )
    .await
    .unwrap();
    ch.queue_declare(
        DLQ,
        QueueDeclareOptions {
            durable: true,
            ..Default::default()
        },
        FieldTable::default(),
    )
    .await
    .unwrap();
    ch.queue_bind(
        DLQ,
        DLX,
        "#",
        QueueBindOptions::default(),
        FieldTable::default(),
    )
    .await
    .unwrap();
    let mut q_args = FieldTable::default();
    q_args.insert(
        "x-dead-letter-exchange".into(),
        lapin::types::AMQPValue::LongString(DLX.into()),
    );
    ch.queue_declare(
        QUEUE,
        QueueDeclareOptions {
            durable: true,
            ..Default::default()
        },
        q_args,
    )
    .await
    .unwrap();
    ch.queue_bind(
        QUEUE,
        EXCHANGE,
        BINDING,
        QueueBindOptions::default(),
        FieldTable::default(),
    )
    .await
    .unwrap();
    drop(ch);

    (url, conn)
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

#[tokio::test]
async fn handler_permanent_goes_to_dlq() {
    let (url, conn) = setup_rabbit().await;

    let body = serde_json::to_vec(&TestPayload { value: 1 }).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Err(HandlerError::Permanent("reject".into()))
        })
        .await
        .unwrap();

    sleep(Duration::from_secs(2)).await;

    let bodies = drain_dlq(&conn).await;
    assert_eq!(bodies.len(), 1, "expected exactly 1 message in DLQ");
    let env: Envelope<TestPayload> = serde_json::from_slice(&bodies[0]).unwrap();
    assert_eq!(env.payload.value, 1);
}

#[tokio::test]
async fn handler_transient_requeues_then_succeeds() {
    let (url, conn) = setup_rabbit().await;
    let counter = Arc::new(AtomicI32::new(0));
    let body = serde_json::to_vec(&TestPayload { value: 7 }).unwrap();
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

    sleep(Duration::from_secs(3)).await;

    let bodies = drain_dlq(&conn).await;
    assert_eq!(bodies.len(), 0, "expected DLQ to be empty after retry success");
    assert_eq!(counter.load(Ordering::SeqCst), 2, "handler should run twice");
}

#[tokio::test]
async fn handler_transient_upgrades_to_dlq_after_3_redeliveries() {
    let (url, conn) = setup_rabbit().await;
    let body = serde_json::to_vec(&TestPayload { value: 9 }).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Err(HandlerError::Transient("always fail".into()))
        })
        .await
        .unwrap();

    sleep(Duration::from_secs(4)).await;

    let bodies = drain_dlq(&conn).await;
    assert_eq!(bodies.len(), 1, "expected 1 message in DLQ after 3 retries");
    let env: Envelope<TestPayload> = serde_json::from_slice(&bodies[0]).unwrap();
    assert_eq!(env.payload.value, 9);
}

#[tokio::test]
async fn deserialization_failure_goes_to_dlq() {
    let (url, conn) = setup_rabbit().await;
    // publish invalid JSON that cannot deserialize to Envelope<TestPayload>
    publish_one(&url, b"not-a-valid-envelope", FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Ok(())
        })
        .await
        .unwrap();

    sleep(Duration::from_secs(2)).await;

    let bodies = drain_dlq(&conn).await;
    assert_eq!(bodies.len(), 1, "deser fail should DLQ");
}

#[tokio::test]
async fn republish_confirm_nack_does_not_ack_original() {
    // 模拟：用一个不存在的 exchange 让 basic_publish 必失败。
    // 但 consumer 的 republish 走同 channel，channel 关闭会让原 delivery 重新投。
    // 简化：在测试中直接构造一个不存在的 routing-key + mandatory=true，让 broker 返 nack。
    // 验证：原 delivery 仍 unacked（队列里有它，consumer 重连后重新可见）。
    todo!("Phase 6-A T3 follow-up: see spec §6.2 item 5")
}
