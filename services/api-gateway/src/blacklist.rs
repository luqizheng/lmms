use redis::{aio::MultiplexedConnection, AsyncCommands};

pub async fn connect(url: &str) -> Result<MultiplexedConnection, redis::RedisError> {
    let client = redis::Client::open(url)?;
    client.get_multiplexed_async_connection().await
}

/// 黑名单 Redis key 契约：`{prefix}blacklist:{jti}`。
///
/// 写入侧（token-service revoke）与读取侧（本 middleware）共用同一拼装；
/// 抽成纯函数供契约测试锁定（WP-TEST-01），防止两侧格式漂移。
pub fn blacklist_key(prefix: &str, jti: &str) -> String {
    format!("{}blacklist:{}", prefix, jti)
}

pub async fn is_blacklisted(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    jti: &str,
) -> Result<bool, redis::RedisError> {
    let key = blacklist_key(prefix, jti);
    conn.exists(&key).await
}
