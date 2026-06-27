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
pub mod publisher;
pub mod publisher_loop;
pub mod routing_keys;

pub use config::EventBusConfig;
pub use envelope::Envelope;
pub use error::{ConsumeError, HandlerError, OutboxError, PublishError, RunError};
pub use event::Event;
pub use events::IamEventEnum;

// Re-exports below are wired as their modules land in later tasks:
// pub use consumer::{Consumer, RabbitMqConsumer};       // Task 10
pub use outbox::{MySqlOutboxRepo, OutboxRepo, OutboxRow}; // Task 8
pub use publisher::{Publisher, RabbitMqPublisher}; // Task 9
// pub use publisher_loop::{OutboxPublisher, OutboxPublisherConfig}; // Task 10