use crate::db::Db;
use crate::models::Status;
use crate::redis_cli::RedisCli;
use crate::ws_hub::{WsHub, WsOut};
use chrono::Utc;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time;

pub async fn run(db: Db, redis: RedisCli, hub: WsHub, tick_secs: u64) {
    let mut iv = time::interval(Duration::from_secs(tick_secs));
    loop {
        iv.tick().await;
        if let Err(e) = tick_once(&db, &redis, &hub).await {
            tracing::error!("scheduler tick failed: {e:?}");
        }
    }
}

async fn tick_once(db: &Db, redis: &RedisCli, hub: &WsHub) -> anyhow::Result<()> {
    let ids = db.due_scheduled(Utc::now()).await?;
    for id in ids {
        if let Err(e) = publish_one(db, redis, hub, id).await {
            tracing::error!("publish {id} failed: {e:?}");
        }
    }
    Ok(())
}

pub async fn publish_one(
    db: &Db,
    redis: &RedisCli,
    hub: &WsHub,
    ann_id: u64,
) -> anyhow::Result<()> {
    let _scopes = db.list_recipient_scopes(ann_id).await?;
    let users: HashSet<u64> = HashSet::new();
    db.set_status(ann_id, Status::Published).await.ok();
    db.insert_deliveries(ann_id, &users).await.ok();
    for u in &users {
        redis.incr_unread(*u, 1).await.ok();
        if let Some(a) = db.get(ann_id).await? {
            hub.publish(*u, WsOut::New(serde_json::json!({
                "id": a.id, "title": a.title, "pinned": a.pinned != 0, "popover": a.popover != 0,
            }))).await;
        }
    }
    Ok(())
}
