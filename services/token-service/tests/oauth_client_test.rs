// requires: cargo test --test-threads=1
mod common;

use token_service::models::oauth_client::Client;
use token_service::service::oauth_client_svc::{
    self, generate_client_id, generate_client_secret, ClientPatch,
};

fn sample_client(id: &str) -> Client {
    Client {
        client_id: id.to_string(),
        client_secret_hash: token_service::secret::hash_client_secret("dev-gateway-secret"),
        name: format!("{id}-name"),
        permissions: vec!["token:issue".into()],
        grant_types: vec!["authorization_code".into()],
        access_token_ttl_sec: 3600,
        refresh_token_ttl_sec: 2592000,
        status: "active".into(),
        user_id: None,
        expires_at: None,
    }
}

#[tokio::test]
async fn seed_then_find_by_id() {
    let pool = common::spawn_mysql().await;
    let n = oauth_client_svc::seed_default_clients(&pool).await.unwrap();
    assert_eq!(n, 4); // api-gateway / token-service / user-service / notification-service

    let c = oauth_client_svc::find_by_id(&pool, "api-gateway")
        .await
        .unwrap();
    assert!(c.is_some());
    let c = c.unwrap();
    assert_eq!(c.client_id, "api-gateway");
    assert!(c.permissions.contains(&"token:issue".to_string()));
}

#[tokio::test]
async fn seed_is_idempotent() {
    let pool = common::spawn_mysql().await;
    let n1 = oauth_client_svc::seed_default_clients(&pool).await.unwrap();
    let n2 = oauth_client_svc::seed_default_clients(&pool).await.unwrap();
    assert_eq!(n1, 4);
    assert_eq!(n2, 0); // 第二次应当 0 affected rows
}

#[tokio::test]
async fn find_unknown_returns_none() {
    let pool = common::spawn_mysql().await;
    oauth_client_svc::seed_default_clients(&pool).await.unwrap();
    let c = oauth_client_svc::find_by_id(&pool, "ghost").await.unwrap();
    assert!(c.is_none());
}

#[tokio::test]
async fn insert_new_client() {
    let pool = common::spawn_mysql().await;
    let c = sample_client("svc-new");
    oauth_client_svc::insert(&pool, &c).await.unwrap();
    let got = oauth_client_svc::find_by_id(&pool, "svc-new")
        .await
        .unwrap();
    assert!(got.is_some());
}

#[tokio::test]
async fn list_with_pagination_and_keyword() {
    let pool = common::spawn_mysql().await;
    oauth_client_svc::seed_default_clients(&pool).await.unwrap();

    // 全量
    let (rows, total) = oauth_client_svc::list(&pool, None, None, 1, 50)
        .await
        .unwrap();
    assert_eq!(total, 4);
    assert_eq!(rows.len(), 4);

    // 关键字 keyword 命中 client_id
    let (rows, total) = oauth_client_svc::list(&pool, Some("token"), None, 1, 50)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(rows[0].client_id, "token-service");

    // status 过滤
    let (_, total_active) = oauth_client_svc::list(&pool, None, Some("active"), 1, 50)
        .await
        .unwrap();
    assert_eq!(total_active, 4);
    let (rows, total_disabled) = oauth_client_svc::list(&pool, None, Some("disabled"), 1, 50)
        .await
        .unwrap();
    assert_eq!(total_disabled, 0);
    assert!(rows.is_empty());

    // 分页：size=2 第二页
    let (rows, total) = oauth_client_svc::list(&pool, None, None, 2, 2)
        .await
        .unwrap();
    assert_eq!(total, 4);
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn update_patches_only_provided_fields() {
    let pool = common::spawn_mysql().await;
    oauth_client_svc::seed_default_clients(&pool).await.unwrap();

    let patch = ClientPatch {
        name: Some("Renamed Gateway".into()),
        permissions: None,
        grant_types: None,
        access_token_ttl_sec: Some(7200),
        refresh_token_ttl_sec: None,
        status: Some("disabled".into()),
    };
    let ok = oauth_client_svc::update(&pool, "api-gateway", &patch)
        .await
        .unwrap();
    assert!(ok);

    let (rows, _) = oauth_client_svc::list(&pool, Some("api-gateway"), None, 1, 10)
        .await
        .unwrap();
    let v = rows.into_iter().next().unwrap();
    assert_eq!(v.name, "Renamed Gateway");
    assert_eq!(v.access_token_ttl_sec, 7200);
    assert_eq!(v.status, "disabled");
    // 未传字段保持原值
    assert!(v.permissions.contains(&"token:issue".to_string()));
}

#[tokio::test]
async fn update_unknown_returns_false() {
    let pool = common::spawn_mysql().await;
    let patch = ClientPatch {
        name: Some("x".into()),
        permissions: None,
        grant_types: None,
        access_token_ttl_sec: None,
        refresh_token_ttl_sec: None,
        status: None,
    };
    let ok = oauth_client_svc::update(&pool, "ghost", &patch)
        .await
        .unwrap();
    assert!(!ok);
}

#[tokio::test]
async fn delete_removes_client() {
    let pool = common::spawn_mysql().await;
    oauth_client_svc::seed_default_clients(&pool).await.unwrap();

    let ok = oauth_client_svc::delete(&pool, "api-gateway")
        .await
        .unwrap();
    assert!(ok);
    let got = oauth_client_svc::find_by_id(&pool, "api-gateway")
        .await
        .unwrap();
    assert!(got.is_none());

    // 再次删除返回 false
    let ok2 = oauth_client_svc::delete(&pool, "api-gateway")
        .await
        .unwrap();
    assert!(!ok2);
}

#[tokio::test]
async fn reset_secret_returns_new_plain_and_changes_hash() {
    let pool = common::spawn_mysql().await;
    oauth_client_svc::seed_default_clients(&pool).await.unwrap();

    let old = oauth_client_svc::find_by_id(&pool, "api-gateway")
        .await
        .unwrap()
        .unwrap();

    let plain = oauth_client_svc::reset_secret(&pool, "api-gateway")
        .await
        .unwrap();
    assert!(plain.is_some());
    let plain = plain.unwrap();
    assert_eq!(plain.len(), 40, "secret 必须为 40 字符");

    let new = oauth_client_svc::find_by_id(&pool, "api-gateway")
        .await
        .unwrap()
        .unwrap();
    assert_ne!(old.client_secret_hash, new.client_secret_hash);
}

#[tokio::test]
async fn reset_secret_unknown_returns_none() {
    let pool = common::spawn_mysql().await;
    let res = oauth_client_svc::reset_secret(&pool, "ghost")
        .await
        .unwrap();
    assert!(res.is_none());
}

#[tokio::test]
async fn insert_client_with_user_id_and_expiry() {
    let pool = common::spawn_mysql().await;
    let mut c = sample_client("svc-bound");
    c.user_id = Some("usr-abc123".to_string());
    c.expires_at = Some(chrono::Utc::now() + chrono::Duration::days(30));
    oauth_client_svc::insert(&pool, &c).await.unwrap();
    let got = oauth_client_svc::find_by_id(&pool, "svc-bound")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.user_id.as_deref(), Some("usr-abc123"));
    assert!(got.expires_at.is_some());
}

#[test]
fn generate_client_secret_has_correct_length() {
    let s = generate_client_secret();
    assert_eq!(s.len(), 40);
    assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn generate_client_id_slugifies_and_appends_suffix() {
    let id = generate_client_id("My Service!");
    let parts: Vec<&str> = id.rsplitn(2, '-').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 6, "随机后缀必须 6 位");
    assert!(id.starts_with("my-service"));

    // 空名称 fallback 为 "client-"
    let id2 = generate_client_id("");
    assert!(id2.starts_with("client-"));
}
