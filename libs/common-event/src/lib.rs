//! digua_common-event: domain event bus primitives
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
pub mod outbox_archive;
pub mod perm_epoch;
pub mod publisher;
pub mod publisher_loop;
pub mod routing_keys;
pub mod span;

pub use config::EventBusConfig;
pub use consumer::{Consumer, RabbitMqConsumer};
pub use envelope::Envelope;
pub use error::{ConsumeError, HandlerError, OutboxError, PublishError, RunError};
pub use event::Event;
pub use events::IamEventEnum;
pub use outbox::{
    claim_batch_sql, enqueue_sql, mark_published_sql, AnyTx, MySqlOutboxRepo, OutboxRepo,
    OutboxRow, SqlOutboxRepo,
};
pub use outbox_archive::run as run_outbox_archive;
pub use outbox_archive::run_once as run_outbox_archive_once;
pub use perm_epoch::{get_perm_epoch, perm_epoch_key, set_perm_epoch_if_greater};
pub use publisher::{Publisher, RabbitMqPublisher};
pub use publisher_loop::{OutboxPublisher, OutboxPublisherOptions};
pub use span::span_from_envelope_trace_id;
