use redis::{aio::MultiplexedConnection, AsyncCommands};

pub async fn connect(url: &str) -> Result<MultiplexedConnection, redis::RedisError> {
    let client = redis::Client::open(url)?;
    client.get_multiplexed_async_connection().await
}

pub async fn set_code(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    code: &str,
    user_id: &str,
    ttl_sec: u64,
) -> Result<(), redis::RedisError> {
    let key = format!("{}code:{}", prefix, code);
    let _: () = conn.set_ex(&key, user_id, ttl_sec).await?;
    Ok(())
}

pub async fn get_code(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    code: &str,
) -> Result<Option<String>, redis::RedisError> {
    let key = format!("{}code:{}", prefix, code);
    conn.get(&key).await
}

pub async fn del_code(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    code: &str,
) -> Result<(), redis::RedisError> {
    let key = format!("{}code:{}", prefix, code);
    let _: () = conn.del(&key).await?;
    Ok(())
}

pub async fn add_to_blacklist(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    jti: &str,
    ttl_sec: u64,
) -> Result<(), redis::RedisError> {
    let key = format!("{}blacklist:{}", prefix, jti);
    let _: () = conn.set_ex(&key, "1", ttl_sec).await?;
    Ok(())
}

pub async fn is_blacklisted(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    jti: &str,
) -> Result<bool, redis::RedisError> {
    let key = format!("{}blacklist:{}", prefix, jti);
    conn.exists(&key).await
}

pub async fn add_refresh_family(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    jti: &str,
    ttl_sec: u64,
) -> Result<bool, redis::RedisError> {
    let key = format!("{}refresh:{}", prefix, jti);
    let _: () = conn.set_ex(&key, "1", ttl_sec).await?;
    Ok(true)
}

pub async fn is_refresh_marked(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    jti: &str,
) -> Result<bool, redis::RedisError> {
    let key = format!("{}refresh:{}", prefix, jti);
    conn.exists(&key).await
}
