//! common-event: domain event bus primitives
//!
//! Provides `Event`, `Envelope`, `Publisher`, `Consumer`, `OutboxRepo` traits
//! with a RabbitMQ implementation. See `phase4-event-bus-design.md` for the
//! overall topology.

pub mod config;
pub mod consumer;
pub mod envelope;
pub mod error;
pub mod event;
pub mod events;
pub mod metrics;
pub mod outbox;
pub mod perm_epoch;
pub mod publisher;
pub mod publisher_loop;
pub mod routing_keys;

pub use config::EventBusConfig;
pub use consumer::{Consumer, RabbitMqConsumer};
pub use envelope::Envelope;
pub use error::{ConsumeError, HandlerError, OutboxError, PublishError, RunError};
pub use event::Event;
pub use events::IamEventEnum;
pub use outbox::{MySqlOutboxRepo, OutboxRepo, OutboxRow};
pub use perm_epoch::{get_perm_epoch, perm_epoch_key, set_perm_epoch_if_greater};
pub use publisher::{Publisher, RabbitMqPublisher};
pub use publisher_loop::{OutboxPublisher, OutboxPublisherOptions};