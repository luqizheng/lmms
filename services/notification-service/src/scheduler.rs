use crate::db::Db;
use crate::iam_client::IamMemberClient;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

pub async fn run(db: Db, iam_client: Arc<IamMemberClient>, tick_secs: u64) {
    let mut iv = time::interval(Duration::from_secs(tick_secs));
    loop {
        iv.tick().await;
        if let Err(e) = tick_once(&db, &iam_client).await {
            tracing::error!("scheduler tick failed: {e:?}");
        }
    }
}

async fn tick_once(db: &Db, iam_client: &IamMemberClient) -> anyhow::Result<()> {
    let ids = db.due_scheduled(Utc::now()).await?;
    for id in ids {
        if let Err(e) = publish_one(db, iam_client, id).await {
            tracing::error!("publish {id} failed: {e:?}");
        }
    }
    Ok(())
}

pub async fn publish_one(db: &Db, iam_client: &IamMemberClient, ann_id: u64) -> anyhow::Result<()> {
    let tenant_id: i64 = sqlx::query_scalar(
        "SELECT tenant_id FROM ntf_announcements WHERE id=?",
    )
    .bind(ann_id)
    .fetch_one(&db.pool)
    .await?;
    db.publish_announcement(ann_id, iam_client, tenant_id).await?;
    Ok(())
}
