use crate::db::Db;
use chrono::Utc;
use std::time::Duration;
use tokio::time;

pub async fn run(db: Db, tick_secs: u64) {
    let mut iv = time::interval(Duration::from_secs(tick_secs));
    loop {
        iv.tick().await;
        if let Err(e) = tick_once(&db).await {
            tracing::error!("scheduler tick failed: {e:?}");
        }
    }
}

async fn tick_once(db: &Db) -> anyhow::Result<()> {
    let ids = db.due_scheduled(Utc::now()).await?;
    for id in ids {
        if let Err(e) = publish_one(db, id).await {
            tracing::error!("publish {id} failed: {e:?}");
        }
    }
    Ok(())
}

pub async fn publish_one(db: &Db, ann_id: u64) -> anyhow::Result<()> {
    db.publish_announcement(ann_id).await?;
    Ok(())
}
