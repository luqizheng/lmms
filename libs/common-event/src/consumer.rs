use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions,
        ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
    Connection, ConnectionProperties, ExchangeKind,
};

use crate::envelope::Envelope;
use crate::error::{ConsumeError, HandlerError};
use crate::event::Event;

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

        let mut consumer = channel
            .basic_consume(
                &queue_name,
                &opts.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| ConsumeError::SubscribeFailed(e.to_string()))?;

        let handler = Arc::new(handler);
        tokio::spawn(async move {
            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(d) => match serde_json::from_slice::<Envelope<P>>(&d.data) {
                        Ok(env) => match handler(env).await {
                            Ok(_) => {
                                let _ = d.ack(BasicAckOptions::default()).await;
                            }
                            Err(_) => {
                                let _ = d
                                    .nack(BasicNackOptions {
                                        requeue: false,
                                        ..Default::default()
                                    })
                                    .await;
                            }
                        },
                        Err(_) => {
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
