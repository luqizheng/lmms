//! gateway 兜底 consumer：订阅 iam.permission.changed，set-if-greater 补写 perm_epoch。
//! 关闭 iam 直写的 fail-open 窗口。使用 event.occurred_at 作为 epoch 值（与 iam 直写同时间基准）。
//!
//! Redis SETEX 失败 → HandlerError::Transient：让 broker 重投（bounded 3 次后升 DLQ）。
//! 之前 swallow 返 Ok 已被 Phase 6-A T3 禁用。
use common_event::events::IamPermissionChanged;
use common_event::perm_epoch::set_perm_epoch_if_greater;
use common_event::{Consumer, ConsumeError, Envelope, HandlerError, RabbitMqConsumer, span_from_envelope_trace_id};
use redis::aio::MultiplexedConnection;
use std::time::Duration;
use tracing::{info, warn, Instrument};

const BINDING: &str = "iam.permission.changed";

pub async fn run(
    conn: MultiplexedConnection,
    prefix: String,
    ttl_sec: u64,
    event_bus_url: String,
) -> Result<(), ConsumeError> {
    let backoff = Duration::from_secs(5);
    loop {
        match try_subscribe(conn.clone(), prefix.clone(), ttl_sec, &event_bus_url).await {
            Ok(consumer) => {
                // Hold the consumer (and thus its ephemeral queue + connection)
                // alive, polling connection health. Re-subscribe ONLY on an
                // actual disconnect — never on a timer — otherwise each cycle
                // would orphan a fresh server-named exclusive queue.
                while consumer.connection().status().connected() {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                warn!("perm_epoch consumer connection lost, resubscribing");
            }
            Err(e) => {
                warn!(error = %e, "perm_epoch consumer subscribe failed, retry in 5s");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_subscribe(
    conn: MultiplexedConnection,
    prefix: String,
    ttl_sec: u64,
    url: &str,
) -> Result<RabbitMqConsumer, ConsumeError> {
    let consumer = RabbitMqConsumer::new(url).await?;
    consumer
        .subscribe_ephemeral::<IamPermissionChanged, _, _>(
            &[BINDING],
            move |env: Envelope<IamPermissionChanged>| {
                let mut conn = conn.clone();
                let prefix = prefix.clone();
                let span = span_from_envelope_trace_id(&env, "gateway.perm-epoch");
                async move {
                    let users: Vec<String> =
                        serde_json::from_value(env.payload.user_ids.clone()).unwrap_or_default();
                    let ts = env.occurred_at.timestamp();
                    for uid in users {
                        set_perm_epoch_if_greater(&mut conn, &prefix, &uid, ts, ttl_sec)
                            .await
                            .map_err(|e| {
                                warn!(error = %e, uid, "perm_epoch backfill failed (will be retried)");
                                HandlerError::Transient(format!("perm_epoch setex: {e}"))
                            })?;
                    }
                    Ok::<(), HandlerError>(())
                }
                .instrument(span)
            },
        )
        .await?;
    info!("perm_epoch consumer subscribed (ephemeral) to {}", BINDING);
    Ok(consumer)
}
