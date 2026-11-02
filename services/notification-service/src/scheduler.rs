use crate::db::Db;
use chrono::Utc;
use digua_common_db::db_fetch_scalar;
use digua_jwt_ctx::TenantScope;
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

pub async fn publish_one(db: &Db, ann_id: i64) -> anyhow::Result<()> {
    // 方言无关标量查询（`db_*!` 宏负责 `?` → `$N` 改写与 bind 计数校验）。
    let tenant_id: i64 = db_fetch_scalar!(
        &db.pool,
        i64,
        "SELECT tenant_id FROM ntf_announcements WHERE id = ?",
        ann_id
    )?;
    let scope = TenantScope::Single(tenant_id);
    db.publish_announcement(ann_id, &scope).await?;
    Ok(())
}
