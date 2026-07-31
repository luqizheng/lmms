// requires: cargo test --test-threads=1
mod digua_common;

use token_service::service::refresh_svc;

#[tokio::test]
async fn mark_then_is_marked() {
    let pool = digua_common::spawn_mysql().await;
    refresh_svc::mark(&pool, "jti-A", 3600).await.unwrap();
    assert!(refresh_svc::is_marked(&pool, "jti-A").await.unwrap());
}

#[tokio::test]
async fn mark_is_idempotent() {
    let pool = digua_common::spawn_mysql().await;
    refresh_svc::mark(&pool, "jti-B", 3600).await.unwrap();
    refresh_svc::mark(&pool, "jti-B", 3600).await.unwrap();
    assert!(refresh_svc::is_marked(&pool, "jti-B").await.unwrap());
}

#[tokio::test]
async fn unknown_jti_not_marked() {
    let pool = digua_common::spawn_mysql().await;
    assert!(!refresh_svc::is_marked(&pool, "nope").await.unwrap());
}
