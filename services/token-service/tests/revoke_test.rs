// requires: cargo test --test-threads=1
mod digua_common;

use token_service::service::revoke_svc;

#[tokio::test]
async fn revoke_then_is_revoked() {
    let pool = digua_common::spawn_mysql().await;
    revoke_svc::revoke(&pool, "jti-1", 3600).await.unwrap();
    assert!(revoke_svc::is_revoked(&pool, "jti-1").await.unwrap());
}

#[tokio::test]
async fn revoke_is_idempotent() {
    let pool = digua_common::spawn_mysql().await;
    revoke_svc::revoke(&pool, "jti-2", 3600).await.unwrap();
    revoke_svc::revoke(&pool, "jti-2", 3600).await.unwrap(); // 不报错
    assert!(revoke_svc::is_revoked(&pool, "jti-2").await.unwrap());
}

#[tokio::test]
async fn unknown_jti_is_not_revoked() {
    let pool = digua_common::spawn_mysql().await;
    assert!(!revoke_svc::is_revoked(&pool, "nope").await.unwrap());
}

#[tokio::test]
async fn expired_revocation_returns_false() {
    let pool = digua_common::spawn_mysql().await;
    revoke_svc::revoke(&pool, "jti-3", 1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    assert!(!revoke_svc::is_revoked(&pool, "jti-3").await.unwrap());
}
