use common_event::events::IamUserCreated;
use common_event::{Envelope, HandlerError};
use sqlx::mysql::MySqlPool;
use tracing::warn;

use crate::redis_cli::RedisCli;
use crate::service::announcement_service::ensure_welcome_for_user;
use crate::ws_hub::WsHub;

pub async fn handle_iam_user_created(
    pool: &MySqlPool,
    redis: &RedisCli,
    hub: &WsHub,
    env: &Envelope<IamUserCreated>,
) -> Result<(), HandlerError> {
    ensure_welcome_for_user(
        pool,
        redis,
        hub,
        env.event_id,
        &env.payload.user_id,
        &env.payload,
        env.occurred_at,
    )
    .await
    .map_err(|e| {
        warn!(error = %e, event_id = %env.event_id, "welcome handler failed");
        HandlerError::Handler(e.to_string())
    })?;
    Ok(())
}