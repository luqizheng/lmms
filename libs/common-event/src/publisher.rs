use async_trait::async_trait;
use lapin::{
    options::{BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};

use crate::envelope::Envelope;
use crate::error::PublishError;
use crate::event::Event;

// clippy 1.99 `double_must_use` 误报源于 async-trait 宏展开（生成的 must_use
// 与 Future 自身重复），仅在宏产生点豁免。
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish<P: Event>(&self, env: &Envelope<P>) -> Result<(), PublishError>;

    /// Publish a pre-serialized envelope body under an explicit routing key.
    /// Used by the outbox publisher loop, where the envelope is stored as raw
    /// JSON and its concrete payload type is not known at compile time.
    /// `message_id` should be the envelope's `event_id` for broker-level tracing.
    async fn publish_raw(
        &self,
        routing_key: &str,
        message_id: &str,
        body: &[u8],
    ) -> Result<(), PublishError>;
}

pub struct RabbitMqPublisher {
    conn: Connection,
    channel: Channel,
    exchange: String,
}

impl RabbitMqPublisher {
    pub async fn new(url: &str, exchange: &str) -> Result<Self, PublishError> {
        let conn = Connection::connect(url, ConnectionProperties::default())
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        let channel = conn
            .create_channel()
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        // Ensure the exchange exists (idempotent — matches pre-declared topology)
        channel
            .exchange_declare(
                exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        Ok(Self {
            conn,
            channel,
            exchange: exchange.to_string(),
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn exchange(&self) -> &str {
        &self.exchange
    }
}

#[async_trait]
impl Publisher for RabbitMqPublisher {
    async fn publish<P: Event>(&self, env: &Envelope<P>) -> Result<(), PublishError> {
        let body = serde_json::to_vec(env)?;
        let confirm = self
            .channel
            .basic_publish(
                &self.exchange,
                P::event_type(),
                BasicPublishOptions {
                    mandatory: true,
                    ..Default::default()
                },
                &body,
                BasicProperties::default()
                    .with_content_type("application/json".into())
                    .with_delivery_mode(2) // persistent
                    .with_message_id(env.event_id.to_string().into()),
            )
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        if confirm.is_nack() {
            return Err(PublishError::Broker("publisher confirm nack".into()));
        }
        Ok(())
    }

    async fn publish_raw(
        &self,
        routing_key: &str,
        message_id: &str,
        body: &[u8],
    ) -> Result<(), PublishError> {
        let confirm = self
            .channel
            .basic_publish(
                &self.exchange,
                routing_key,
                BasicPublishOptions {
                    mandatory: true,
                    ..Default::default()
                },
                body,
                BasicProperties::default()
                    .with_content_type("application/json".into())
                    .with_delivery_mode(2)
                    .with_message_id(message_id.to_string().into()),
            )
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        if confirm.is_nack() {
            return Err(PublishError::Broker("publisher confirm nack".into()));
        }
        Ok(())
    }
}
