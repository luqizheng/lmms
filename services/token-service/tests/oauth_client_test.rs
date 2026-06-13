// requires: cargo test --test-threads=1
mod common;

use token_service::models::oauth_client::Client;
use token_service::service::oauth_client_svc;

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
