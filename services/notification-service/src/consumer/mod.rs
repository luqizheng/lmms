pub mod iam_events;
pub mod placeholder;

use std::time::Duration;

use common_event::events::IamUserCreated;
use common_event::{Consumer, Envelope, RabbitMqConsumer};
use sqlx::mysql::MySqlPool;
use tracing::{info, warn};

use crate::redis_cli::RedisCli;
use crate::ws_hub::WsHub;

const QUEUE_NAME: &str = "notification.iam-events";
const BINDING: &str = "iam.user.created";

pub async fn run(
    pool: MySqlPool,
    redis: RedisCli,
    hub: WsHub,
    event_bus_url: String,
) -> anyhow::Result<()> {
    let backoff = Duration::from_secs(5);
    loop {
        match try_subscribe(pool.clone(), redis.clone(), hub.clone(), &event_bus_url).await {
            Ok(()) => {
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
            Err(e) => {
                warn!(error = %e, "notification consumer subscribe failed, retry in 5s");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_subscribe(
    pool: MySqlPool,
    redis: RedisCli,
    hub: WsHub,
    event_bus_url: &str,
) -> anyhow::Result<()> {
    let consumer = RabbitMqConsumer::new(event_bus_url).await?;
    consumer
        .subscribe::<IamUserCreated, _, _>(
            QUEUE_NAME,
            &[BINDING],
            move |env: Envelope<IamUserCreated>| {
                let pool = pool.clone();
                let redis = redis.clone();
                let hub = hub.clone();
                async move { iam_events::handle_iam_user_created(&pool, &redis, &hub, &env).await }
            },
        )
        .await?;
    info!("notification consumer subscribed to {} ({})", QUEUE_NAME, BINDING);
    Ok(())
}