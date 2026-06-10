use redis::{Client, AsyncCommands};

fn get_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

fn get_test_prefix() -> String {
    "test:token-server:".to_string()
}

#[tokio::test]
async fn test_code_set_and_get() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let code = "test-auth-code-001";
    let user_id = "user-123";
    let ttl_sec: u64 = 300;

    let key = format!("{}code:{}", prefix, code);
    let _: () = conn.set_ex(&key, user_id, ttl_sec).await.unwrap();

    let stored: Option<String> = conn.get(&key).await.unwrap();
    assert_eq!(stored, Some(user_id.to_string()));

    let _: () = conn.del(&key).await.unwrap();
}

#[tokio::test]
async fn test_code_expiration() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let code = "test-auth-code-002";
    let ttl_sec: u64 = 1;

    let key = format!("{}code:{}", prefix, code);
    let _: () = conn.set_ex(&key, "user-123", ttl_sec).await.unwrap();

    let exists_immediate: bool = conn.exists(&key).await.unwrap();
    assert!(exists_immediate);

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let exists_after: bool = conn.exists(&key).await.unwrap();
    assert!(!exists_after);
}

#[tokio::test]
async fn test_code_delete_after_use() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let code = "test-auth-code-003";

    let key = format!("{}code:{}", prefix, code);
    let _: () = conn.set_ex(&key, "user-456", 300).await.unwrap();

    let _: () = conn.del(&key).await.unwrap();

    let stored: Option<String> = conn.get(&key).await.unwrap();
    assert_eq!(stored, None);
}

#[tokio::test]
async fn test_blacklist_add_and_check() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let jti = "revoked-jti-001";
    let ttl_sec: u64 = 3600;

    let key = format!("{}blacklist:{}", prefix, jti);
    let _: () = conn.set_ex(&key, "1", ttl_sec).await.unwrap();

    let exists: bool = conn.exists(&key).await.unwrap();
    assert!(exists);

    let _: () = conn.del(&key).await.unwrap();
}

#[tokio::test]
async fn test_refresh_family_tracking() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let family = "family-123";
    let ttl_sec: u64 = 86400;

    let key = format!("{}refresh:{}", prefix, family);
    let _: () = conn.set_ex(&key, "1", ttl_sec).await.unwrap();

    let exists: bool = conn.exists(&key).await.unwrap();
    assert!(exists);

    let is_marked: bool = conn.exists(&key).await.unwrap();
    assert!(is_marked);

    let _: () = conn.del(&key).await.unwrap();

    let exists_after: bool = conn.exists(&key).await.unwrap();
    assert!(!exists_after);
}

#[tokio::test]
async fn test_nonexistent_code_returns_none() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let nonexistent_code = "nonexistent-code-xyz";

    let key = format!("{}code:{}", prefix, nonexistent_code);
    let stored: Option<String> = conn.get(&key).await.unwrap();
    assert_eq!(stored, None);
}

#[tokio::test]
async fn test_multiple_codes_isolation() {
    let client = Client::open(get_redis_url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let prefix = get_test_prefix();
    let codes = vec![
        ("code-001", "user-1"),
        ("code-002", "user-2"),
        ("code-003", "user-3"),
    ];

    for (code, user_id) in &codes {
        let key = format!("{}code:{}", prefix, code);
        let _: () = conn.set_ex(&key, *user_id, 300).await.unwrap();
    }

    for (code, expected_user) in &codes {
        let key = format!("{}code:{}", prefix, code);
        let stored: Option<String> = conn.get(&key).await.unwrap();
        assert_eq!(stored, Some(expected_user.to_string()));
    }

    let keys: Vec<String> = codes.iter()
        .map(|(code, _)| format!("{}code:{}", prefix, code))
        .collect();
    let _: () = conn.del(keys).await.unwrap();

    for (code, _) in &codes {
        let key = format!("{}code:{}", prefix, code);
        let stored: Option<String> = conn.get(&key).await.unwrap();
        assert_eq!(stored, None);
    }
}
