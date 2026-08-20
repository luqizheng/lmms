// requires: cargo test --test-threads=1
// svc 层端到端：auth_code → refresh marker → revoke → oauth_client（不经 HTTP）
mod common;

use token_service::service::{auth_code_svc, oauth_client_svc, refresh_svc, revoke_svc};

#[tokio::test]
async fn e2e_access_refresh_revoke() {
    let pool = common::spawn_mysql().await;
    oauth_client_svc::seed_default_clients(&pool).await.unwrap();

    // 1. seed an auth code
    auth_code_svc::save(&pool, "auth-code-1", "user-42", 3600)
        .await
        .unwrap();

    // 2. consume it
    let u = auth_code_svc::consume(&pool, "auth-code-1").await.unwrap();
    assert_eq!(u, Some("user-42".into()));

    // 3. issue a refresh marker
    refresh_svc::mark(&pool, "refresh-jti-1", 3600)
        .await
        .unwrap();
    assert!(refresh_svc::is_marked(&pool, "refresh-jti-1")
        .await
        .unwrap());

    // 4. revoke a jti
    revoke_svc::revoke(&pool, "access-jti-1", 3600)
        .await
        .unwrap();
    assert!(revoke_svc::is_revoked(&pool, "access-jti-1").await.unwrap());

    // 5. find api-gateway client
    let c = oauth_client_svc::find_by_id(&pool, "api-gateway")
        .await
        .unwrap();
    assert!(c.is_some());
}
