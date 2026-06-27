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

        channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;

        for rk in bindings {
            channel
                .queue_bind(
                    queue,
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
                queue,
                "audit-service",
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
