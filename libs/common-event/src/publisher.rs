use async_trait::async_trait;
use lapin::{
    options::{BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};

use crate::envelope::Envelope;
use crate::error::PublishError;
use crate::event::Event;

#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish<P: Event>(&self, env: &Envelope<P>) -> Result<(), PublishError>;
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
}
