use notification_service::config::AppConfig;
use notification_service::db::Db;
use notification_service::models::{CreateAnnouncement, ScopeInput, ScopeType, Status};
use notification_service::redis_cli::RedisCli;

#[tokio::test]
#[ignore = "requires MySQL/Redis containers; run with --include-ignored"]
async fn create_then_publish_then_read_flow() {
    let cfg = AppConfig::load(std::path::Path::new("config/config.yaml")).unwrap();
    let db = Db::connect(&cfg.mysql.url, cfg.mysql.max_connections)
        .await
        .unwrap();
    let r = RedisCli::connect(&cfg.redis.url).await.unwrap();
    let id = db
        .create_announcement(
            &CreateAnnouncement {
                title: "T".into(),
                body: "B".into(),
                publish_at: chrono::Utc::now() - chrono::Duration::seconds(1),
                expire_at: None,
                pinned: true,
                popover: true,
                recipients: vec![ScopeInput {
                    scope_type: ScopeType::User,
                    scope_id: "42".into(),
                }],
            },
            "9001".into(),
        )
        .await
        .unwrap();
    db.set_status(id, Status::Published).await.unwrap();
    let changed = db.mark_read(id, "42".into()).await.unwrap();
    assert!(changed);
    r.decr_unread_floor0("42".into()).await.unwrap();
    assert!(r.get_unread("42".into()).await.unwrap() <= 0);
}
