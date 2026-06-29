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
    /// 瞬时错误，broker 重投。超过阈值（consumer 内固定 3）后升级 DLQ。
    /// 典型：下游 DB / Redis 抖动、临时网络错误、429/503。
    #[error("transient: {0}")]
    Transient(String),

    /// 永久错误，立即 DLQ。
    /// 典型：业务校验失败、消息载荷不合法、引用资源已删。
    #[error("permanent: {0}")]
    Permanent(String),
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