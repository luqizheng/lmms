use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
        BasicQosOptions, ConfirmSelectOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::{AMQPValue, FieldTable},
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};

use crate::envelope::Envelope;
use crate::error::{ConsumeError, HandlerError};
use crate::event::Event;

const REDELIVERY_LIMIT: i64 = 3;
const REDELIVERY_HEADER: &str = "x-redelivery-count";

/// Read the broker header `x-redelivery-count` from a delivery's properties.
/// Returns 0 if the header is missing or malformed (first delivery).
#[doc(hidden)]
pub fn get_redelivery_count(headers: &Option<FieldTable>) -> i64 {
    headers
        .as_ref()
        .and_then(|h| h.inner().get(REDELIVERY_HEADER))
        .and_then(|v| match v {
            AMQPValue::LongLongInt(n) => Some(*n),
            AMQPValue::LongInt(n) => Some(*n as i64),
            _ => None,
        })
        .unwrap_or(0)
}

/// Republish `body` to the same routing-key on the same exchange, with
/// `x-redelivery-count` header incremented by 1.
///
/// Uses `wait_for_confirms` to both wait for the publisher confirm AND
/// drain any `Basic.Return` messages (mandatory publish to a missing
/// exchange or unroutable routing-key). Returns `Err` on publish failure,
/// confirm nack, or unroutable return — caller MUST NOT ack the original
/// delivery in that case.
#[doc(hidden)]
pub async fn republish_with_incremented_count(
    channel: &Channel,
    exchange: &str,
    routing_key: &str,
    body: &[u8],
    old_count: i64,
    message_id: Option<&str>,
) -> Result<(), lapin::Error> {
    let mut headers = FieldTable::default();
    headers.insert(
        REDELIVERY_HEADER.into(),
        AMQPValue::LongLongInt(old_count + 1),
    );
    let mut props = BasicProperties::default()
        .with_content_type("application/json".into())
        .with_delivery_mode(2) // persistent
        .with_headers(headers);
    if let Some(mid) = message_id {
        props = props.with_message_id(mid.to_string().into());
    }

    // Keep Confirmation alive so the confirm stays registered while
    // wait_for_confirms polls it. Dropped after wait_for_confirms returns.
    let _confirmation = channel
        .basic_publish(
            exchange,
            routing_key,
            BasicPublishOptions {
                mandatory: true,
                ..Default::default()
            },
            body,
            props,
        )
        .await?;

    // wait_for_confirms resolves all pending publisher confirms (Err on nack)
    // and drains Basic.Return messages (unroutable). Single-publish channel
    // so there is exactly one pending confirm.
    //
    // Race: Basic.Return can arrive after the confirm ack but before the
    // drain inside wait_for_confirms. We poll once more after a short sleep
    // to catch any stragglers (T3-fu1).
    let mut returns = channel
        .wait_for_confirms()
        .await
        .map_err(|e| std::io::Error::other(format!("publisher confirm: {e}")))?;

    if returns.is_empty() {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let more = channel
            .wait_for_confirms()
            .await
            .map_err(|e| std::io::Error::other(format!("publisher confirm: {e}")))?;
        returns.extend(more);
    }

    if !returns.is_empty() {
        return Err(std::io::Error::other("message unroutable (Basic.Return received)").into());
    }
    Ok(())
}

/// Tuning knobs for how a queue is declared and consumed.
///
/// `subscribe` (durable, DLX-backed, shared queue) and `subscribe_ephemeral`
/// (server-named, exclusive, auto-delete, no DLX) are thin wrappers that build
/// one of these and delegate to the shared `subscribe_inner` boilerplate.
#[derive(Debug, Clone)]
pub struct SubscribeOptions {
    pub durable: bool,
    pub exclusive: bool,
    pub auto_delete: bool,
    pub with_dlx: bool,
    pub consumer_tag: String,
}

// clippy 1.99 `double_must_use` 误报源于 async-trait 宏展开（生成的 must_use
// 与 Future 自身重复），仅在宏产生点豁免。
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait Consumer: Send + Sync {
    async fn subscribe<P, H, Fut>(
        &self,
        queue: &str,
        bindings: &[&str],
        handler: H,
    ) -> Result<(), ConsumeError>
    where
        P: Event,
        H: Fn(Envelope<P>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), HandlerError>> + Send;

    /// Subscribe via a per-replica EPHEMERAL queue: server-named, exclusive,
    /// auto-delete, non-durable, no DLX. Each subscriber gets its own queue,
    /// so a fanout binding delivers every message to ALL subscribers
    /// (fanout-to-all) rather than load-balancing across them.
    async fn subscribe_ephemeral<P, H, Fut>(
        &self,
        bindings: &[&str],
        handler: H,
    ) -> Result<(), ConsumeError>
    where
        P: Event,
        H: Fn(Envelope<P>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), HandlerError>> + Send;
}

pub struct RabbitMqConsumer {
    conn: Connection,
    exchange: String,
}

impl RabbitMqConsumer {
    pub async fn new(url: &str) -> Result<Self, ConsumeError> {
        Self::with_exchange(url, "events").await
    }

    pub async fn with_exchange(url: &str, exchange: &str) -> Result<Self, ConsumeError> {
        let conn = Connection::connect(url, ConnectionProperties::default())
            .await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;
        Ok(Self {
            conn,
            exchange: exchange.to_string(),
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Shared subscribe boilerplate: create channel + qos, declare the topic
    /// exchange, declare the queue per `opts`, bind it, start consuming, and
    /// spawn the delivery loop. `subscribe` and `subscribe_ephemeral` differ
    /// only in the `SubscribeOptions` (and the queue name) they pass here.
    ///
    /// When `queue` is `""` the broker assigns a server-generated name; we
    /// bind/consume using the actual name returned by `queue_declare`.
    async fn subscribe_inner<P, H, Fut>(
        &self,
        queue: &str,
        bindings: &[&str],
        opts: SubscribeOptions,
        handler: H,
    ) -> Result<(), ConsumeError>
    where
        P: Event,
        H: Fn(Envelope<P>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), HandlerError>> + Send,
    {
        let channel = self
            .conn
            .create_channel()
            .await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;
        channel
            .basic_qos(32, BasicQosOptions::default())
            .await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;

        // Ensure the topic exchange exists (idempotent — matches pre-declared
        // topology, but lets a consumer start before any publisher has run).
        channel
            .exchange_declare(
                &self.exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;

        // Declare the queue. For durable shared queues we add the same
        // x-dead-letter-exchange arg as the pre-declared topology in
        // definitions.json so a redeclare doesn't fail with
        // PRECONDITION_FAILED. The DLX exchange itself isn't required to exist
        // at declare time — RabbitMQ only validates the arg type/match, and
        // routes through the DLX when a message is actually dead-lettered.
        let mut queue_args = FieldTable::default();
        if opts.with_dlx {
            queue_args.insert(
                "x-dead-letter-exchange".into(),
                lapin::types::AMQPValue::LongString("events.dlx".into()),
            );
        }
        let declared = channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    durable: opts.durable,
                    exclusive: opts.exclusive,
                    auto_delete: opts.auto_delete,
                    ..Default::default()
                },
                queue_args,
            )
            .await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;

        // For server-named ("") queues, use the broker-assigned name for
        // binding + consuming; otherwise this equals `queue`.
        let queue_name = declared.name().as_str().to_string();

        for rk in bindings {
            channel
                .queue_bind(
                    &queue_name,
                    &self.exchange,
                    rk,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(|e| ConsumeError::Broker(e.to_string()))?;
        }

        let consumer = channel
            .basic_consume(
                &queue_name,
                &opts.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| ConsumeError::SubscribeFailed(e.to_string()))?;

        let handler = Arc::new(handler);
        let exchange_name = self.exchange.clone();
        let channel_clone = channel.clone();
        tokio::spawn(async move {
            let mut consumer = consumer;
            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(d) => match serde_json::from_slice::<Envelope<P>>(&d.data) {
                        Ok(env) => {
                            let count = get_redelivery_count(d.properties.headers());
                            let routing_key = d.routing_key.as_str().to_string();
                            let body = d.data.clone();
                            let mid = d.properties.message_id().as_ref().map(|m| m.to_string());
                            match handler(env).await {
                                Ok(_) => {
                                    let _ = d.ack(BasicAckOptions::default()).await;
                                }
                                Err(HandlerError::Permanent(msg)) => {
                                    tracing::warn!(error = %msg, "handler permanent error → DLQ");
                                    let _ = d
                                        .nack(BasicNackOptions {
                                            requeue: false,
                                            ..Default::default()
                                        })
                                        .await;
                                }
                                Err(HandlerError::Transient(msg)) => {
                                    if count >= REDELIVERY_LIMIT {
                                        tracing::warn!(
                                            error = %msg,
                                            count,
                                            "transient exceeded redelivery limit → DLQ"
                                        );
                                        let _ = d
                                            .nack(BasicNackOptions {
                                                requeue: false,
                                                ..Default::default()
                                            })
                                            .await;
                                    } else {
                                        match republish_with_incremented_count(
                                            &channel_clone,
                                            &exchange_name,
                                            &routing_key,
                                            &body,
                                            count,
                                            mid.as_deref(),
                                        )
                                        .await
                                        {
                                            Ok(_) => {
                                                tracing::info!(
                                                    error = %msg,
                                                    count = count + 1,
                                                    "transient → republished"
                                                );
                                                let _ = d.ack(BasicAckOptions::default()).await;
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    error = %e,
                                                    "republish failed; leaving orig for redelivery"
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "envelope deserialization failed → DLQ");
                            let _ = d
                                .nack(BasicNackOptions {
                                    requeue: false,
                                    ..Default::default()
                                })
                                .await;
                        }
                    },
                    Err(_) => break,
                }
            }
        });
        Ok(())
    }
}

#[async_trait]
impl Consumer for RabbitMqConsumer {
    async fn subscribe<P, H, Fut>(
        &self,
        queue: &str,
        bindings: &[&str],
        handler: H,
    ) -> Result<(), ConsumeError>
    where
        P: Event,
        H: Fn(Envelope<P>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), HandlerError>> + Send,
    {
        self.subscribe_inner(
            queue,
            bindings,
            SubscribeOptions {
                durable: true,
                exclusive: false,
                auto_delete: false,
                with_dlx: true,
                consumer_tag: format!("{}-{}", queue, uuid::Uuid::new_v4()),
            },
            handler,
        )
        .await
    }

    async fn subscribe_ephemeral<P, H, Fut>(
        &self,
        bindings: &[&str],
        handler: H,
    ) -> Result<(), ConsumeError>
    where
        P: Event,
        H: Fn(Envelope<P>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), HandlerError>> + Send,
    {
        self.subscribe_inner(
            "",
            bindings,
            SubscribeOptions {
                durable: false,
                exclusive: true,
                auto_delete: true,
                with_dlx: false,
                consumer_tag: format!("ephemeral-{}", uuid::Uuid::new_v4()),
            },
            handler,
        )
        .await
    }
}
