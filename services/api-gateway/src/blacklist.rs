use redis::{aio::MultiplexedConnection, AsyncCommands};

pub async fn connect(url: &str) -> Result<MultiplexedConnection, redis::RedisError> {
    let client = redis::Client::open(url)?;
    client.get_multiplexed_async_connection().await
}

pub async fn is_blacklisted(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    jti: &str,
) -> Result<bool, redis::RedisError> {
    let key = format!("{}blacklist:{}", prefix, jti);
    conn.exists(&key).await
}
