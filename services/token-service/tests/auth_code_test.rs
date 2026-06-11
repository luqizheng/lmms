// requires: cargo test --test-threads=1
mod common;

use token_service::service::auth_code_svc;

#[tokio::test]
async fn save_then_consume_succeeds_once() {
    let pool = common::spawn_mysql().await;
    auth_code_svc::save(&pool, "code-1", "user-1", 3600)
        .await
        .unwrap();
    let u = auth_code_svc::consume(&pool, "code-1").await.unwrap();
    assert_eq!(u, Some("user-1".to_string()));
}

#[tokio::test]
async fn second_consume_returns_none() {
    let pool = common::spawn_mysql().await;
    auth_code_svc::save(&pool, "code-2", "user-2", 3600)
        .await
        .unwrap();
    let _ = auth_code_svc::consume(&pool, "code-2").await.unwrap();
    let u2 = auth_code_svc::consume(&pool, "code-2").await.unwrap();
    assert_eq!(u2, None);
}

#[tokio::test]
async fn consume_expired_returns_none() {
    let pool = common::spawn_mysql().await;
    auth_code_svc::save(&pool, "code-3", "user-3", 1)
        .await
        .unwrap();
    // 让它过期
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let u = auth_code_svc::consume(&pool, "code-3").await.unwrap();
    assert_eq!(u, None);
}

#[tokio::test]
async fn consume_unknown_returns_none() {
    let pool = common::spawn_mysql().await;
    let u = auth_code_svc::consume(&pool, "nope").await.unwrap();
    assert_eq!(u, None);
}

#[tokio::test]
async fn concurrent_consume_only_one_succeeds() {
    let pool = common::spawn_mysql().await;
    auth_code_svc::save(&pool, "code-4", "user-4", 3600)
        .await
        .unwrap();
    let p1 = pool.clone();
    let p2 = pool.clone();
    let (r1, r2) = tokio::join!(
        auth_code_svc::consume(&p1, "code-4"),
        auth_code_svc::consume(&p2, "code-4"),
    );
    let r1 = r1.unwrap();
    let r2 = r2.unwrap();
    let hits = [&r1, &r2].iter().filter(|x| x.is_some()).count();
    assert_eq!(hits, 1, "exactly one consumer must win");
}
