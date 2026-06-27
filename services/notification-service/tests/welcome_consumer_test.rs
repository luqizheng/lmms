use chrono::Utc;
use common_event::events::IamUserCreated;
use notification_service::config::AppConfig;
use notification_service::db::Db;
use notification_service::redis_cli::RedisCli;
use notification_service::service::announcement_service::ensure_welcome_for_user;
use notification_service::ws_hub::WsHub;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires MySQL/Redis containers; run with --include-ignored"]
async fn welcome_idempotent_and_renders_template() {
    let cfg = AppConfig::load(std::path::Path::new("config/config.yaml")).unwrap();
    let db = Db::connect(&cfg.mysql.url, cfg.mysql.max_connections)
        .await
        .unwrap();
    db.ensure_schema().await.unwrap();
    let redis = RedisCli::connect(&cfg.redis.url).await.unwrap();
    let hub = WsHub::new();

    let event_id = Uuid::now_v7();
    let user_id = format!("test-user-{}", &event_id.to_string()[..8]);
    let payload = IamUserCreated {
        user_id: user_id.clone(),
        username: "Alice".to_string(),
        email: Some("alice@example.com".to_string()),
        org_unit_id: Some("ou-eng".to_string()),
    };

    let created = ensure_welcome_for_user(
        &db.pool,
        &redis,
        &hub,
        event_id,
        &user_id,
        &payload,
        Utc::now(),
    )
    .await
    .unwrap();
    assert!(created, "first call should create the announcement");

    let again = ensure_welcome_for_user(
        &db.pool,
        &redis,
        &hub,
        event_id,
        &user_id,
        &payload,
        Utc::now(),
    )
    .await
    .unwrap();
    assert!(!again, "same event_id must be idempotent");

    let list = db.list_for_user(user_id.clone()).await.unwrap();
    assert!(!list.is_empty(), "user should have at least one announcement");
    let last = list.first().unwrap();
    assert!(last.title.contains("Alice"));
    assert!(last.body.contains("ou-eng"));
}
