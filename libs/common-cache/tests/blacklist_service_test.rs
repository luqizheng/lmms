//! WP-TEST-01-R1 · DEC-02：TokenBlacklistService 业务逻辑回归（PURE/MEMORY）。
//!
//! 存储替身 = `MemoryCache`（InMemory）+ 测试内 `FailingCache`（错误注入）。
//! 注入时钟让"过期"用例零 sleep、完全确定。
//!
//! ⚠ 覆盖边界（DEC-02 第 11 条）：本文件**不**验证 Redis TTL 回收、
//! 值序列化、网络连接或任何真实 Redis 行为——这些只能由
//! services/api-gateway/tests/blacklist_test.rs（REDIS 类，需
//! REDIS_URL）覆盖。这里验证的是业务层契约：key 拼装、撤销/查询/
//! 逻辑过期/隔离语义，以及存储错误如实传播（不吞错、不谎报撤销状态）。

use async_trait::async_trait;
use digua_common_cache::{blacklist_key, Cache, MemoryCache, NowMsFn, TokenBlacklistService};
use digua_common_error::{AppError, AppResult};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 可推进时钟：返回 (NowMsFn, 推进句柄)。
fn clock_at(ms: i64) -> (NowMsFn, Arc<Mutex<i64>>) {
    let cell = Arc::new(Mutex::new(ms));
    let reader = cell.clone();
    let now = Arc::new(move || *reader.lock().unwrap()) as NowMsFn;
    (now, cell)
}

fn service(now: NowMsFn, prefix: &str) -> TokenBlacklistService {
    TokenBlacklistService::with_clock(Arc::new(MemoryCache::new(1024)), prefix, now)
}

/// 恒错存储：模拟 Redis 读/写故障。
struct FailingCache;

#[async_trait]
impl Cache for FailingCache {
    async fn get(&self, _key: &str) -> AppResult<Option<Vec<u8>>> {
        Err(AppError::internal(
            "CACHE_REDIS_GET",
            "injected read failure",
        ))
    }
    async fn set(&self, _key: &str, _val: Vec<u8>, _ttl: Option<Duration>) -> AppResult<()> {
        Err(AppError::internal(
            "CACHE_REDIS_SET",
            "injected write failure",
        ))
    }
    async fn del(&self, _key: &str) -> AppResult<()> {
        Err(AppError::internal(
            "CACHE_REDIS_DEL",
            "injected delete failure",
        ))
    }
}

// ── revoke / is_revoked / 过期 ──────────────────────────────────────────────

#[tokio::test]
async fn revokes_then_reports_revoked() {
    let (now, _) = clock_at(1_000);
    let svc = service(now, "t:");
    svc.revoke("jti-a", Duration::from_secs(60)).await.unwrap();
    assert!(svc.is_revoked("jti-a").await.unwrap());
}

#[tokio::test]
async fn unknown_jti_is_not_revoked() {
    let (now, _) = clock_at(1_000);
    let svc = service(now, "t:");
    assert!(!svc.is_revoked("never-issued").await.unwrap());
}

#[tokio::test]
async fn expires_when_clock_passes_until_boundary() {
    let (now, cell) = clock_at(1_000);
    let svc = service(now, "t:");
    svc.revoke("jti-b", Duration::from_millis(100))
        .await
        .unwrap();

    // 未到期：撤销中。
    *cell.lock().unwrap() = 1_099;
    assert!(svc.is_revoked("jti-b").await.unwrap());
    // 到点即失效：now == until。
    *cell.lock().unwrap() = 1_100;
    assert!(!svc.is_revoked("jti-b").await.unwrap());
    // 过期后仍未被重复撤销的其他 jti 不受影响。
    assert!(!svc.is_revoked("jti-other").await.unwrap());
}

#[tokio::test]
async fn re_revoke_extends_until() {
    let (now, cell) = clock_at(1_000);
    let svc = service(now, "t:");
    svc.revoke("jti-c", Duration::from_millis(100))
        .await
        .unwrap();
    *cell.lock().unwrap() = 1_080; // 第一次撤销临近到期
    svc.revoke("jti-c", Duration::from_millis(100))
        .await
        .unwrap(); // 续期
    *cell.lock().unwrap() = 1_150; // 已越过第一次的 until(1100)
    assert!(svc.is_revoked("jti-c").await.unwrap()); // 第二次 until=1180 仍撤销中
}

// ── 隔离 ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn different_jtis_are_isolated() {
    let (now, _) = clock_at(1_000);
    let svc = service(now, "p:");
    svc.revoke("jti-1", Duration::from_secs(60)).await.unwrap();
    assert!(svc.is_revoked("jti-1").await.unwrap());
    assert!(!svc.is_revoked("jti-2").await.unwrap());
}

#[tokio::test]
async fn different_prefixes_do_not_collide() {
    let (now, _) = clock_at(1_000);
    let s1 = service(now.clone(), "tenant1:");
    let s2 = service(now, "tenant2:");
    s1.revoke("shared-jti", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(s1.is_revoked("shared-jti").await.unwrap());
    assert!(!s2.is_revoked("shared-jti").await.unwrap());
}

// ── key 契约（写入侧/读取侧共用） ───────────────────────────────────────────

#[tokio::test]
async fn writes_under_the_shared_key_contract() {
    // 服务内部写入的 key 必须与 blacklist_key 契约一致——直接在底层
    // MemoryCache 上按契约 key 查询必须命中。
    let (now, _) = clock_at(1_000);
    let store = Arc::new(MemoryCache::new(1024));
    let svc = TokenBlacklistService::with_clock(store.clone(), "pre:", now);
    svc.revoke("jti-k", Duration::from_secs(60)).await.unwrap();
    let hit = Cache::get(store.as_ref(), &blacklist_key("pre:", "jti-k"))
        .await
        .unwrap();
    assert!(
        hit.is_some(),
        "服务必须写入契约 key {{prefix}}blacklist:{{jti}}"
    );
    let miss = Cache::get(store.as_ref(), &blacklist_key("pre:", "jti-other"))
        .await
        .unwrap();
    assert!(miss.is_none());
}

// ── 错误策略（DEC-02 第 6 条）：如实传播，不吞错、不谎报撤销状态 ────────────

#[tokio::test]
async fn read_failure_propagates_instead_of_silently_denying_or_allowing() {
    let (now, _) = clock_at(1_000);
    let svc = TokenBlacklistService::with_clock(Arc::new(FailingCache), "t:", now);
    let res = svc.is_revoked("jti-x").await;
    // 业务服务不得把存储故障伪装成"未撤销(false)"或"已撤销(true)"——
    // 必须返回错误，由调用方按既有安全策略处置。
    assert!(res.is_err(), "存储读故障必须如实上抛");
}

#[tokio::test]
async fn write_failure_propagates_instead_of_reporting_revocation_success() {
    let (now, _) = clock_at(1_000);
    let svc = TokenBlacklistService::with_clock(Arc::new(FailingCache), "t:", now);
    let res = svc.revoke("jti-x", Duration::from_secs(60)).await;
    // 业务服务不得在存储写故障时谎报"撤销成功"。
    assert!(res.is_err(), "存储写故障必须如实上抛");
}

#[tokio::test]
async fn corrupt_value_propagates_error_not_false() {
    // 值损坏（非本服务写入的格式）不得被当作"未撤销"放行——报错交由
    // 调用方策略处置。
    let (now, _) = clock_at(1_000);
    let store = Arc::new(MemoryCache::new(1024));
    let svc = TokenBlacklistService::with_clock(store.clone(), "t:", now);
    Cache::set(
        store.as_ref(),
        &blacklist_key("t:", "jti-bad"),
        b"not-a-timestamp".to_vec(),
        None,
    )
    .await
    .unwrap(); // 模拟非本服务格式/人为写坏的值（注意：旧版写入的纯数字
               // 值如 "1" 会被当作极早的过期时刻 → 视为未撤销，见
               // R1 报告"滚动部署旧值"风险备案，本用例锁定的是非数字
               // 值必须报错而非默认放行）
    let res = svc.is_revoked("jti-bad").await;
    assert!(res.is_err(), "损坏值必须报错而非默认放行");
}

// ── 系统时钟冒烟（生产构造路径可用性） ──────────────────────────────────────

#[tokio::test]
async fn system_clock_constructor_smoke() {
    let svc = TokenBlacklistService::new(Arc::new(MemoryCache::new(64)), "smoke:");
    svc.revoke("jti-live", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(svc.is_revoked("jti-live").await.unwrap());
}
