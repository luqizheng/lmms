use thiserror::Error;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("broker error: {0}")]
    Broker(String),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("not connected")]
    NotConnected,
}

#[derive(Debug, Error)]
pub enum ConsumeError {
    #[error("broker error: {0}")]
    Broker(String),
    #[error("subscribe failed: {0}")]
    SubscribeFailed(String),
}

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("handler error: {0}")]
    Handler(String),
}

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("not found: event_id={0}")]
    NotFound(String),
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("publish loop terminated: {0}")]
    Publish(String),
    #[error("shutdown timeout")]
    ShutdownTimeout,
}