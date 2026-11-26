//! REDIS 类集成测试（WP-TEST-01-R1 · DEC-02 第 7/10 条）：`SharedRedisCache`
//! （生产 blacklist 适配器）对真实 Redis 的 get/set_ex(ttl)/del 往返验证。
//!
//! 不在 test:logic 白名单（已列入 scripts/test-logic.js 的 INFRA_TEST_FILES），
//! 常规 CI 不执行。本地执行：
//!
//! ```bash
//! pnpm run dev:infra   # 或任一可用 Redis
//! REDIS_URL=redis://127.0.0.1:36379 \
//!   cargo test -p digua_common_cache --features redis --test shared_redis_cache_test
//! ```
//!
//! 未设置 REDIS_URL 时打印 [skip] 并通过（不连接任何服务）。
//! ⚠ 本文件验证真实 Redis 行为（TTL/网络）；MEMORY 类的业务逻辑回归在
//! blacklist_service_test.rs，两者不得互相替代。

#![cfg(feature = "redis")]

use digua_common_cache::{Cache, SharedRedisCache};
use std::sync::Arc;
use std::time::Duration;

fn redis_url() -> Option<String> {
    std::env::var("REDIS_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

async fn connect(url: &str) -> Result<Arc<dyn Cache>, redis::RedisError> {
    let client = redis::Client::open(url)?;
    let conn = client.get_multiplexed_async_connection().await?;
    Ok(Arc::new(SharedRedisCache::new(conn)))
}

#[tokio::test]
async fn set_get_del_roundtrip_over_real_redis() {
    let Some(url) = redis_url() else {
        eprintln!("[skip] REDIS_URL not set — SharedRedisCache integration test skipped");
        return;
    };
    let cache = connect(&url).await.unwrap();

    let key = "test:shared-cache:roundtrip";
    cache.del(key).await.unwrap();
    cache.set(key, b"v1".to_vec(), None).await.unwrap();
    let hit = cache.get(key).await.unwrap();
    assert_eq!(hit.as_deref(), Some(b"v1".as_slice()));
    cache.del(key).await.unwrap();
    let miss = cache.get(key).await.unwrap();
    assert!(miss.is_none());
}

#[tokio::test]
async fn set_ex_entry_expires_on_real_redis() {
    let Some(url) = redis_url() else {
        eprintln!("[skip] REDIS_URL not set — SharedRedisCache integration test skipped");
        return;
    };
    let cache = connect(&url).await.unwrap();

    let key = "test:shared-cache:ttl";
    cache
        .set(key, b"gone".to_vec(), Some(Duration::from_secs(1)))
        .await
        .unwrap();
    let live = cache.get(key).await.unwrap();
    assert_eq!(live.as_deref(), Some(b"gone".as_slice()));

    // 用独立连接查 TTL：SET_EX 必须在 Redis 侧设置真实过期。
    let client = redis::Client::open(url.as_str()).unwrap();
    let mut probe = client.get_multiplexed_async_connection().await.unwrap();
    let ttl: i64 = redis::cmd("TTL")
        .arg(key)
        .query_async(&mut probe)
        .await
        .unwrap();
    assert!(
        (1..=2).contains(&ttl),
        "SET_EX 必须在 Redis 侧设置真实 TTL（实际 TTL={ttl}s）"
    );
    cache.del(key).await.unwrap();
}
