//! 风控 consumer：独立订阅 iam.login.failed，按 username/IP 滑动窗口计数，
//! 越线时打 WARN 日志 + 递增 Prometheus 计数器。仅计数+告警，不锁号、不通知。
//!
//! Redis 出错时返 HandlerError::Transient 让 broker 重投（bounded redelivery 3 次后
//! 升级 DLQ），不再 swallow 返 Ok——Phase 6-A T3 起 swallow 已被禁用。计数不做
//! event_id 幂等去重（至少一次投递下偶发偏高可接受）。

use std::time::Duration;

use common_event::events::IamLoginFailed;
use common_event::{Consumer, Envelope, HandlerError, RabbitMqConsumer};
use prometheus::{register_int_counter_vec, IntCounterVec};
use std::sync::OnceLock;
use tracing::{info, warn, Instrument};

use crate::consumer_span::span_from_envelope_trace_id;
use crate::redis_cli::RedisCli;

const QUEUE_NAME: &str = "audit.login-risk";
const BINDING: &str = "iam.login.failed";

static LOGIN_RISK_ALERTS: OnceLock<IntCounterVec> = OnceLock::new();

pub fn init_metrics() {
    let counter = register_int_counter_vec!(
        "login_risk_alerts_total",
        "Total login-failure risk threshold crossings",
        &["dimension"]
    )
    .expect("register_int_counter_vec login_risk_alerts_total failed");
    LOGIN_RISK_ALERTS.get_or_init(|| counter);
}

#[derive(Debug, Clone, Copy)]
pub struct RiskOptions {
    pub threshold: i64,
    pub window_secs: usize,
}

/// 窗口内只在「恰好等于阈值」那一次告警，避免刷屏。
fn crosses_threshold(count: i64, threshold: i64) -> bool {
    count == threshold
}

async fn count_dimension(
    redis: &RedisCli,
    dimension: &str,
    key_value: &str,
    opts: &RiskOptions,
) -> Result<(), HandlerError> {
    let key = format!("audit:loginfail:{dimension}:{key_value}");
    match redis.incr_with_window(&key, opts.window_secs).await {
        Ok(n) => {
            if crosses_threshold(n, opts.threshold) {
                warn!(
                    dimension,
                    key_value,
                    count = n,
                    window_secs = opts.window_secs,
                    "login failure threshold exceeded"
                );
                if let Some(c) = LOGIN_RISK_ALERTS.get() {
                    c.with_label_values(&[dimension]).inc();
                }
            }
            Ok(())
        }
        Err(e) => {
            warn!(error = %e, dimension, key_value, "risk counter redis error (will be retried)");
            Err(HandlerError::Transient(format!("redis incr: {e}")))
        }
    }
}

async fn handle(redis: &RedisCli, env: &Envelope<IamLoginFailed>, opts: &RiskOptions) -> Result<(), HandlerError> {
    let p = &env.payload;
    if !p.username.is_empty() {
        count_dimension(redis, "user", &p.username, opts).await?;
    }
    if !p.ip.is_empty() {
        count_dimension(redis, "ip", &p.ip, opts).await?;
    }
    Ok(())
}

pub async fn run(redis: RedisCli, event_bus_url: String, opts: RiskOptions) -> anyhow::Result<()> {
    let backoff = Duration::from_secs(5);
    loop {
        match try_subscribe(redis.clone(), &event_bus_url, opts).await {
            Ok(()) => tokio::time::sleep(Duration::from_secs(30)).await,
            Err(e) => {
                warn!(error = %e, "risk consumer subscribe failed, retry in 5s");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_subscribe(redis: RedisCli, event_bus_url: &str, opts: RiskOptions) -> anyhow::Result<()> {
    let consumer = RabbitMqConsumer::new(event_bus_url).await?;
    consumer
        .subscribe::<IamLoginFailed, _, _>(
            QUEUE_NAME,
            &[BINDING],
            move |env: Envelope<IamLoginFailed>| {
                let redis = redis.clone();
                async move {
                    let span = span_from_envelope_trace_id(&env, "audit.login-risk");
                    async move { handle(&redis, &env, &opts).await }
                        .instrument(span)
                        .await
                }
            },
        )
        .await?;
    info!("risk consumer subscribed to {} ({})", QUEUE_NAME, BINDING);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerts_only_at_threshold() {
        assert!(!crosses_threshold(1, 5));
        assert!(!crosses_threshold(4, 5));
        assert!(crosses_threshold(5, 5));
        assert!(!crosses_threshold(6, 5)); // 不重复告警
    }
}
