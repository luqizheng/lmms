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
        let mut conn = self.client.get_async_connection().await
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
                conn.expire(&lock_key, ttl.as_secs() as usize).await
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
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        
        let script = redis::Script::new(
            "if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('del', KEYS[1]) else return 0 end"
        );
        script.key(&self.key).arg("").invoke_async(&mut conn).await
            .map_err(|e| AppError::internal("CACHE_REDIS_UNLOCK", e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        let result: Option<Vec<u8>> = conn.get(key).await
            .map_err(|e| AppError::internal("CACHE_REDIS_GET", e.to_string()))?;
        Ok(result)
    }

    async fn set(&self, key: &str, val: Vec<u8>, ttl: Option<Duration>) -> AppResult<()> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        
        if let Some(t) = ttl {
            conn.set_ex(key, val, t.as_secs() as usize).await
                .map_err(|e| AppError::internal("CACHE_REDIS_SET", e.to_string()))?;
        } else {
            conn.set(key, val).await
                .map_err(|e| AppError::internal("CACHE_REDIS_SET", e.to_string()))?;
        }
        Ok(())
    }

    async fn del(&self, key: &str) -> AppResult<()> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        conn.del(key).await
            .map_err(|e| AppError::internal("CACHE_REDIS_DEL", e.to_string()))?;
        Ok(())
    }

    async fn incr(&self, key: &str) -> AppResult<i64> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        let result: i64 = conn.incr(key, 1).await
            .map_err(|e| AppError::internal("CACHE_REDIS_INCR", e.to_string()))?;
        Ok(result)
    }

    async fn mget(&self, keys: &[&str]) -> AppResult<Vec<Option<Vec<u8>>>> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AppError::internal("CACHE_REDIS_CONN", e.to_string()))?;
        let result: Vec<Option<Vec<u8>>> = conn.mget(keys).await
            .map_err(|e| AppError::internal("CACHE_REDIS_MGET", e.to_string()))?;
        Ok(result)
    }
}