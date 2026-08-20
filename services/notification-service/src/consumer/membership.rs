//! Phase 5B-fu3: durable consumer for `notification.membership-events` queue.
//!
//! Routing keys bound:
//! - `iam.user.role.assigned`
//! - `iam.user.role.unassigned`
//! - `iam.user.dept.changed`
//!
//! 三个 typed subscriber 共享同一 durable queue（durable queue + 多 bindings，
//! 每条消息只会按 routing-key 投递一次；这里三次 subscribe 是同一 queue，broker
//! 会自动 dedupe 同一 consumer-tag / channel 的重复 basic_consume，故需用
//! 一次性 `subscribe_inner` 风格的合并 handler。改实现：用一个 `Value` payload
//! 的 single subscribe + 内部按 envelope.event_type dispatch — 但 Value 的
//! event_type() 是 "*"，丢 routing_key 信息，所以改方案）。
//!
//! 最终方案：三次 typed subscribe，每次绑到 routing_key 子集；consumer-tag
//! 各异 → 三次 basic_consume 各自 ack/nack，独立计数。
//!
//! 错误分级（Phase 6-A T3 沿用）：
//! - DB 抖动 → `HandlerError::Transient` → republish-and-ack 阈值 3 → DLQ
//! - envelope schema 错 / 字段缺失 → `HandlerError::Permanent` → DLQ

use std::time::Duration;

use digua_common_event::events::{IamUserDeptChanged, IamUserRoleAssigned, IamUserRoleUnassigned};
use digua_common_event::{
    span_from_envelope_trace_id, Consumer, Envelope, HandlerError, RabbitMqConsumer,
};
use tracing::{error, info, warn, Instrument};

use crate::db::Db;

const QUEUE_NAME: &str = "notification.membership-events";

pub async fn run(db: Db, event_bus_url: String) -> anyhow::Result<()> {
    let backoff = Duration::from_secs(5);
    loop {
        match try_subscribe(db.clone(), &event_bus_url).await {
            Ok(consumer) => {
                // Block on disconnect — see Phase 4.3-fu4 rationale: avoid
                // re-subscribing on a healthy broker (durable queue +
                // binding are idempotent but pure churn).
                while consumer.connection().status().connected() {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                warn!("membership consumer connection lost, resubscribing");
            }
            Err(e) => {
                warn!(error = %e, "membership consumer subscribe failed, retry in 5s");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_subscribe(db: Db, event_bus_url: &str) -> anyhow::Result<RabbitMqConsumer> {
    let consumer = RabbitMqConsumer::new(event_bus_url).await?;
    let db_assigned = db.clone();
    consumer
        .subscribe::<IamUserRoleAssigned, _, _>(
            QUEUE_NAME,
            &["iam.user.role.assigned"],
            move |env: Envelope<IamUserRoleAssigned>| {
                let db = db_assigned.clone();
                let span = span_from_envelope_trace_id(&env, "notification.membership-events");
                async move { handle_role_assigned(&db, &env).await }.instrument(span)
            },
        )
        .await?;
    let db_unassigned = db.clone();
    consumer
        .subscribe::<IamUserRoleUnassigned, _, _>(
            QUEUE_NAME,
            &["iam.user.role.unassigned"],
            move |env: Envelope<IamUserRoleUnassigned>| {
                let db = db_unassigned.clone();
                let span = span_from_envelope_trace_id(&env, "notification.membership-events");
                async move { handle_role_unassigned(&db, &env).await }.instrument(span)
            },
        )
        .await?;
    let db_dept = db.clone();
    consumer
        .subscribe::<IamUserDeptChanged, _, _>(
            QUEUE_NAME,
            &["iam.user.dept.changed"],
            move |env: Envelope<IamUserDeptChanged>| {
                let db = db_dept.clone();
                let span = span_from_envelope_trace_id(&env, "notification.membership-events");
                async move { handle_dept_changed(&db, &env).await }.instrument(span)
            },
        )
        .await?;
    info!(
        "membership consumer subscribed to {} (3 typed subscriptions)",
        QUEUE_NAME
    );
    Ok(consumer)
}

async fn handle_role_assigned(
    db: &Db,
    env: &Envelope<IamUserRoleAssigned>,
) -> Result<(), HandlerError> {
    let p = &env.payload;
    db.apply_role_assigned(&p.user_id, &p.role_id, p.tenant_id, p.ts)
        .await
        .map_err(|e| {
            error!(error = %e, user_id = %p.user_id, role_id = %p.role_id, "apply_role_assigned failed");
            HandlerError::Transient(e.to_string())
        })
}

async fn handle_role_unassigned(
    db: &Db,
    env: &Envelope<IamUserRoleUnassigned>,
) -> Result<(), HandlerError> {
    let p = &env.payload;
    db.apply_role_unassigned(&p.user_id, &p.role_id, p.tenant_id, p.ts)
        .await
        .map_err(|e| {
            error!(error = %e, user_id = %p.user_id, role_id = %p.role_id, "apply_role_unassigned failed");
            HandlerError::Transient(e.to_string())
        })
}

async fn handle_dept_changed(
    db: &Db,
    env: &Envelope<IamUserDeptChanged>,
) -> Result<(), HandlerError> {
    let p = &env.payload;
    db.apply_dept_changed(&p.user_id, p.dept_id.as_deref(), p.tenant_id, p.ts)
        .await
        .map_err(|e| {
            error!(error = %e, user_id = %p.user_id, dept_id = ?p.dept_id, "apply_dept_changed failed");
            HandlerError::Transient(e.to_string())
        })
}
