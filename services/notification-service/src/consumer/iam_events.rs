use common_event::events::IamUserCreated;
use common_event::{Envelope, HandlerError, MySqlOutboxRepo};
use sqlx::mysql::MySqlPool;
use tracing::warn;

use crate::redis_cli::RedisCli;
use crate::service::announcement_service::ensure_welcome_for_user;

pub async fn handle_iam_user_created(
    pool: &MySqlPool,
    redis: &RedisCli,
    outbox_repo: &MySqlOutboxRepo,
    env: &Envelope<IamUserCreated>,
) -> Result<(), HandlerError> {
    // ensure_welcome_for_user 已内部用 INSERT IGNORE / ON DUPLICATE KEY 吸收幂等冲突；
    // 任何漏出的 sqlx 错误都视为 DB 抖动（瞬时），让 broker 重投（bounded 3 次后升 DLQ）。
    ensure_welcome_for_user(
        pool,
        redis,
        outbox_repo,
        env.event_id,
        &env.payload.user_id,
        &env.payload,
        env.occurred_at,
    )
    .await
    .map_err(|e| {
        warn!(error = %e, event_id = %env.event_id, "welcome handler failed");
        HandlerError::Transient(e.to_string())
    })?;
    Ok(())
}
