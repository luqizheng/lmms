use crate::Cache;
use async_trait::async_trait;
use digua_common_error::{AppError, AppResult};
use redis::{AsyncCommands, Client};
use std::time::Duration;

pub struct RedisCache {
    client: Client,
}

impl RedisCache {
    pub async fn new(url: &str) -> AppResult<Self> {
        let client = Client::open(url)
            .map_err(|e| AppError::internal("CACHE_REDIS_CONNECT", e.to_string()))?;
        Ok(Self { client })
    }

    pub async fn lock(&self, key: &str, ttl: Duration) -> AppResult<DistributedLock> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;

        let lock_key = format!("lock:{}", key);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let lock_until = now + ttl.as_millis() as i64;
        let value = lock_until.to_string();

        let result: Result<redis::Value, _> = conn.set_nx(&lock_key, &value).await;
        match result {
            Ok(redis::Value::Int(1)) => {
                let _: () = conn
                    .expire(&lock_key, ttl.as_secs() as i64)
                    .await
                    .map_err(|e| AppError::internal("CACHE_REDIS_EXPIRE", e.to_string()))?;
                Ok(DistributedLock {
                    client: self.client.clone(),
                    key: lock_key,
                })
            }
            Ok(_) => Err(AppError::conflict("CACHE_LOCKED", "resource locked")),
            Err(e) => Err(AppError::internal("CACHE_REDIS_LOCK", e.to_string())),
        }
    }
}

pub struct DistributedLock {
    client: Client,
    key: String,
}

impl DistributedLock {
    pub async fn unlock(self) -> AppResult<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;

        let script = redis::Script::new(
            "if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('del', KEYS[1]) else return 0 end"
        );
        let _: () = script
            .key(&self.key)
            .arg("")
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_UNLOCK", e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        let result: Option<Vec<u8>> = conn
            .get(key)
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_GET", e.to_string()))?;
        Ok(result)
    }

    async fn set(&self, key: &str, val: Vec<u8>, ttl: Option<Duration>) -> AppResult<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;

        if let Some(t) = ttl {
            let _: () = conn
                .set_ex(key, val, t.as_secs())
                .await
                .map_err(|e| AppError::internal("CACHE_REDIS_SET", e.to_string()))?;
        } else {
            let _: () = conn
                .set(key, val)
                .await
                .map_err(|e| AppError::internal("CACHE_REDIS_SET", e.to_string()))?;
        }
        Ok(())
    }

    async fn del(&self, key: &str) -> AppResult<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        let _: () = conn
            .del(key)
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_DEL", e.to_string()))?;
        Ok(())
    }

    async fn incr(&self, key: &str) -> AppResult<i64> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        let result: i64 = conn
            .incr(key, 1)
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_INCR", e.to_string()))?;
        Ok(result)
    }

    async fn mget(&self, keys: &[&str]) -> AppResult<Vec<Option<Vec<u8>>>> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        let result: Vec<Option<Vec<u8>>> = conn
            .mget(keys)
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_MGET", e.to_string()))?;
        Ok(result)
    }
}

/// 复用调用方既有 Redis 连接的 [`Cache`] 实现（`MultiplexedConnection`
/// 或 `ConnectionManager` 均可）。
///
/// 与 [`RedisCache`]（每次操作新建连接）不同：本实现 clone 内部共享连接
/// （廉价、复用同一 TCP），适配 gateway/iam 启动期已建连的模式
/// （WP-TEST-01-R1 · DEC-02：blacklist 读取/写入路径复用现有连接）。
pub struct SharedRedisCache<C: redis::aio::ConnectionLike + Clone + Send + Sync + 'static> {
    conn: C,
}

impl<C: redis::aio::ConnectionLike + Clone + Send + Sync + 'static> SharedRedisCache<C> {
    pub fn new(conn: C) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl<C: redis::aio::ConnectionLike + Clone + Send + Sync + 'static> Cache for SharedRedisCache<C> {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        let mut conn = self.conn.clone();
        let result: Option<Vec<u8>> = conn
            .get(key)
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_GET", e.to_string()))?;
        Ok(result)
    }

    async fn set(&self, key: &str, val: Vec<u8>, ttl: Option<Duration>) -> AppResult<()> {
        let mut conn = self.conn.clone();
        if let Some(t) = ttl {
            let _: () = conn
                .set_ex(key, val, t.as_secs())
                .await
                .map_err(|e| AppError::internal("CACHE_REDIS_SET", e.to_string()))?;
        } else {
            let _: () = conn
                .set(key, val)
                .await
                .map_err(|e| AppError::internal("CACHE_REDIS_SET", e.to_string()))?;
        }
        Ok(())
    }

    async fn del(&self, key: &str) -> AppResult<()> {
        let mut conn = self.conn.clone();
        let _: () = conn
            .del(key)
            .await
            .map_err(|e| AppError::internal("CACHE_REDIS_DEL", e.to_string()))?;
        Ok(())
    }
}
