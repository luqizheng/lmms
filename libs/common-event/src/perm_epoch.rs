//! Shared perm-epoch primitive (Phase 4.4).
//! Key: `{prefix}perm_epoch:{user_id}` = unix-seconds of the user's last
//! permission change. Writers (iam direct + gateway consumer backfill) MUST
//! use set-if-greater so out-of-order writes never lower the epoch (which
//! would resurrect already-invalidated tokens). gateway compares token.iat
//! against this; iat < epoch ⇒ token issued before the change ⇒ reject.
use redis::aio::ConnectionLike;

pub fn perm_epoch_key(prefix: &str, user_id: &str) -> String {
    format!("{prefix}perm_epoch:{user_id}")
}

const SET_IF_GREATER: &str = r#"
local cur = redis.call('GET', KEYS[1])
if (not cur) or (tonumber(cur) < tonumber(ARGV[1])) then
  redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[2])
end
return 1
"#;

pub async fn set_perm_epoch_if_greater<C: ConnectionLike + Send>(
    conn: &mut C,
    prefix: &str,
    user_id: &str,
    ts: i64,
    ttl_sec: u64,
) -> Result<(), redis::RedisError> {
    let key = perm_epoch_key(prefix, user_id);
    redis::Script::new(SET_IF_GREATER)
        .key(key)
        .arg(ts)
        .arg(ttl_sec as i64)
        .invoke_async::<i64>(conn)
        .await?;
    Ok(())
}

pub async fn get_perm_epoch<C: ConnectionLike + Send>(
    conn: &mut C,
    prefix: &str,
    user_id: &str,
) -> Result<Option<i64>, redis::RedisError> {
    let key = perm_epoch_key(prefix, user_id);
    redis::cmd("GET").arg(key).query_async(conn).await
}
