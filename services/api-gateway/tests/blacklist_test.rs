use redis::{AsyncCommands, Client};

fn get_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

fn get_test_prefix() -> String {
    "test:api-gateway:blacklist:".to_string()
}

#[tokio::test]
async fn test_blacklist_set_and_exists() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let jti = "test-jti-blacklist-001";

    let key = format!("{}blacklist:{}", prefix, jti);
    let _: () = conn.set_ex(&key, "1", 60u64).await.unwrap();

    let exists: bool = conn.exists(&key).await.unwrap();
    assert!(exists);

    let _: () = conn.del(&key).await.unwrap();

    let exists_after: bool = conn.exists(&key).await.unwrap();
    assert!(!exists_after);
}

#[tokio::test]
async fn test_blacklist_ttl_expiration() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let jti = "test-jti-blacklist-002";

    let key = format!("{}blacklist:{}", prefix, jti);
    let _: () = conn.set_ex(&key, "1", 1u64).await.unwrap();

    let exists_immediate: bool = conn.exists(&key).await.unwrap();
    assert!(exists_immediate);

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let exists_after_ttl: bool = conn.exists(&key).await.unwrap();
    assert!(!exists_after_ttl);
}

#[tokio::test]
async fn test_blacklist_prefix_isolation() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix1 = "test:prefix1:";
    let prefix2 = "test:prefix2:";
    let jti = "shared-jti";

    let key1 = format!("{}blacklist:{}", prefix1, jti);
    let key2 = format!("{}blacklist:{}", prefix2, jti);

    let _: () = conn.set_ex(&key1, "1", 300u64).await.unwrap();
    let _: () = conn.set_ex(&key2, "1", 300u64).await.unwrap();

    let exists1: bool = conn.exists(&key1).await.unwrap();
    let exists2: bool = conn.exists(&key2).await.unwrap();

    assert!(exists1);
    assert!(exists2);

    let _: () = conn.del(&key1).await.unwrap();
    let _: () = conn.del(&key2).await.unwrap();
}

#[tokio::test]
async fn test_blacklist_multiple_tokens() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let jtis = vec!["test-jti-001", "test-jti-002", "test-jti-003"];

    for jti in &jtis {
        let key = format!("{}blacklist:{}", prefix, jti);
        let _: () = conn.set_ex(&key, "1", 300u64).await.unwrap();
    }

    for jti in &jtis {
        let key = format!("{}blacklist:{}", prefix, jti);
        let exists: bool = conn.exists(&key).await.unwrap();
        assert!(exists, "Token {} should be blacklisted", jti);
    }

    let _: () = conn
        .del(
            jtis
                .iter()
                .map(|jti| format!("{}blacklist:{}", prefix, jti))
                .collect::<Vec<_>>(),
        )
        .await
        .unwrap();

    for jti in &jtis {
        let key = format!("{}blacklist:{}", prefix, jti);
        let exists: bool = conn.exists(&key).await.unwrap();
        assert!(
            !exists,
            "Token {} should not be blacklisted after cleanup",
            jti
        );
    }
}

#[tokio::test]
async fn test_blacklist_false_positive() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let jti_not_blacklisted = "test-jti-not-blacklisted";
    let jti_blacklisted = "test-jti-blacklisted";

    let key_blacklisted = format!("{}blacklist:{}", prefix, jti_blacklisted);
    let _: () = conn.set_ex(&key_blacklisted, "1", 300u64).await.unwrap();

    let exists_not_blacklisted: bool = conn
        .exists(format!("{}blacklist:{}", prefix, jti_not_blacklisted))
        .await
        .unwrap();
    let exists_blacklisted: bool = conn.exists(&key_blacklisted).await.unwrap();

    assert!(!exists_not_blacklisted);
    assert!(exists_blacklisted);

    let _: () = conn.del(&key_blacklisted).await.unwrap();
}
