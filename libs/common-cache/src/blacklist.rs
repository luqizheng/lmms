//! Token blacklist 业务服务（WP-TEST-01-R1 · DEC-02）。
//!
//! 撤销名单的业务逻辑（revoke / is_revoked / 过期判断 / key 契约）与
//! 具体存储解耦：本模块只依赖既有 [`Cache`] 抽象。
//!
//! - 生产：`SharedRedisCache`（真实 Redis，redis feature）
//! - 常规 CI：`MemoryCache`（InMemory，无任何基础设施）+ 注入时钟
//!
//! 过期语义在本层实现（value 编码过期时刻 + 读取时判断），**不依赖、
//! 也不冒充存储层 TTL 行为**——Redis TTL 回收/持久化语义只能由真实
//! Redis 集成测试（services/api-gateway/tests/blacklist_test.rs，REDIS
//! 类）覆盖，Mock 测试不得声称覆盖之。
//!
//! key 契约（`{prefix}blacklist:{jti}`）的唯一真相源在本模块：
//! 写入侧（iam-service logout）与读取侧（api-gateway middleware）共用。

use crate::Cache;
use digua_common_error::{AppError, AppResult};
use std::sync::Arc;
use std::time::Duration;

/// 黑名单 Redis key 契约：`{prefix}blacklist:{jti}`。
///
/// 写入侧（iam-service revoke）与读取侧（api-gateway middleware）共用
/// 同一拼装；格式漂移会让已撤销 token 通过鉴权。
pub fn blacklist_key(prefix: &str, jti: &str) -> String {
    format!("{}blacklist:{}", prefix, jti)
}

/// 可注入时钟（毫秒 Unix epoch），供 InMemory 测试确定性推进时间。
pub type NowMsFn = Arc<dyn Fn() -> i64 + Send + Sync>;

fn system_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Token 撤销名单业务服务：薄封装 [`Cache`]，与存储实现解耦。
#[derive(Clone)]
pub struct TokenBlacklistService {
    store: Arc<dyn Cache>,
    prefix: String,
    now_ms: NowMsFn,
}

impl TokenBlacklistService {
    /// 生产构造（系统时钟）。
    pub fn new(store: Arc<dyn Cache>, prefix: impl Into<String>) -> Self {
        Self::with_clock(store, prefix, Arc::new(system_now_ms))
    }

    /// 测试构造：注入时钟，让"过期"用例零 sleep、完全确定。
    pub fn with_clock(store: Arc<dyn Cache>, prefix: impl Into<String>, now_ms: NowMsFn) -> Self {
        Self {
            store,
            prefix: prefix.into(),
            now_ms,
        }
    }

    /// 撤销 `jti`，有效期 `ttl`。存储侧同时收到 TTL（真实 Redis 用它做
    /// 条目回收）；过期判断以本层编码的过期时刻为准，不依赖存储 TTL。
    pub async fn revoke(&self, jti: &str, ttl: Duration) -> AppResult<()> {
        let until = (self.now_ms)() + ttl.as_millis() as i64;
        self.store
            .set(
                &blacklist_key(&self.prefix, jti),
                until.to_string().into_bytes(),
                Some(ttl),
            )
            .await
    }

    /// `jti` 是否已被撤销且尚未过期。存储读取失败或值损坏时**如实返回
    /// 错误**（不吞错、不默认放行、不谎报撤销状态）——错误后的放行/拒绝
    /// 策略由调用方中间件按既有安全策略执行（api-gateway：fail-open +
    /// warn，2026-07-14 1910c405 决策）。
    pub async fn is_revoked(&self, jti: &str) -> AppResult<bool> {
        match self.store.get(&blacklist_key(&self.prefix, jti)).await? {
            None => Ok(false),
            Some(bytes) => {
                let text = String::from_utf8(bytes)
                    .map_err(|e| AppError::internal("BLACKLIST_CORRUPT_VALUE", e.to_string()))?;
                let until: i64 = text.trim().parse().map_err(|e| {
                    AppError::internal("BLACKLIST_CORRUPT_VALUE", format!("{text:?}: {e}"))
                })?;
                // 边界：到点即失效（now == until 视为过期）。
                Ok((self.now_ms)() < until)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryCache;
    use std::sync::Mutex;

    /// 共享可推进时钟：`Arc<Mutex<i64>>` + 闭包读取。
    fn clock_at(ms: i64) -> (NowMsFn, Arc<Mutex<i64>>) {
        let cell = Arc::new(Mutex::new(ms));
        let cell2 = cell.clone();
        let now = Arc::new(move || *cell2.lock().unwrap()) as NowMsFn;
        (now, cell)
    }

    #[tokio::test]
    async fn revoke_then_revoked_then_expires_with_injected_clock() {
        let (now, cell) = clock_at(1_000);
        let store = Arc::new(MemoryCache::new(1024));
        let svc = TokenBlacklistService::with_clock(store, "t:", now);

        svc.revoke("jti-1", Duration::from_millis(100))
            .await
            .unwrap();
        assert!(svc.is_revoked("jti-1").await.unwrap());

        *cell.lock().unwrap() = 1_099;
        assert!(svc.is_revoked("jti-1").await.unwrap());

        *cell.lock().unwrap() = 1_100; // 到点即失效
        assert!(!svc.is_revoked("jti-1").await.unwrap());
    }
}
