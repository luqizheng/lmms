use redis::aio::MultiplexedConnection;

pub async fn connect(url: &str) -> Result<MultiplexedConnection, redis::RedisError> {
    let client = redis::Client::open(url)?;
    client.get_multiplexed_async_connection().await
}

/// 黑名单 Redis key 契约：`{prefix}blacklist:{jti}`。
///
/// 唯一真相源在 `digua_common_cache::blacklist`（WP-TEST-01-R1 · DEC-02）：
/// 写入侧（iam-service logout）与读取侧（本 middleware 的
/// `TokenBlacklistService`）共用同一实现，re-export 仅为保持既有契约
/// 测试（tests/blacklist_key_test.rs）的导入路径稳定。
pub use digua_common_cache::blacklist_key;
