use digua_common_cache::{Cache, MemoryCache, TieredCache};

#[derive(Clone)]
struct MockRemoteCache {
    data: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>>,
}

impl MockRemoteCache {
    fn new() -> Self {
        Self {
            data: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl Cache for MockRemoteCache {
    async fn get(&self, key: &str) -> digua_common_error::AppResult<Option<Vec<u8>>> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }

    async fn set(
        &self,
        key: &str,
        val: Vec<u8>,
        _ttl: Option<std::time::Duration>,
    ) -> digua_common_error::AppResult<()> {
        self.data.lock().unwrap().insert(key.to_string(), val);
        Ok(())
    }

    async fn del(&self, key: &str) -> digua_common_error::AppResult<()> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }
}

#[tokio::test]
async fn tiered_cache_local_hit() {
    let remote = Box::new(MockRemoteCache::new()) as Box<dyn Cache>;

    let tiered = TieredCache::new(100, remote);
    // tiered.set 写穿（local + remote）→ 后续 get 命中 local 层。
    // 注：不能像旧版那样自建 MemoryCache 直接 set —— TieredCache::new
    // 总是自建 local，外部实例与它无关（pre-existing test bug）。
    tiered
        .set("key1", "local_value".as_bytes().to_vec(), None)
        .await
        .unwrap();

    let result = tiered.get("key1").await.unwrap();
    assert_eq!(result, Some("local_value".as_bytes().to_vec()));
}

#[tokio::test]
async fn tiered_cache_remote_hit_populates_local() {
    let _local = MemoryCache::new(100);
    let remote = Box::new(MockRemoteCache::new()) as Box<dyn Cache>;
    remote
        .set("key2", "remote_value".as_bytes().to_vec(), None)
        .await
        .unwrap();

    let tiered = TieredCache::new(100, remote);

    let result = tiered.get("key2").await.unwrap();
    assert_eq!(result, Some("remote_value".as_bytes().to_vec()));
}

#[tokio::test]
async fn tiered_cache_set_propagates_to_both() {
    let mock = MockRemoteCache::new();
    let remote = Box::new(mock.clone()) as Box<dyn Cache>;
    let remote_check = Box::new(mock.clone()) as Box<dyn Cache>;

    let tiered = TieredCache::new(100, remote);

    tiered
        .set("key3", "both_value".as_bytes().to_vec(), None)
        .await
        .unwrap();

    // local 层：tiered.get 直接命中（值已在内部 local）。
    assert_eq!(
        tiered.get("key3").await.unwrap(),
        Some("both_value".as_bytes().to_vec())
    );
    // remote 层：mock 收到了写穿。
    assert_eq!(
        remote_check.get("key3").await.unwrap(),
        Some("both_value".as_bytes().to_vec())
    );
}

#[tokio::test]
async fn tiered_cache_del_propagates_to_both() {
    let mock = MockRemoteCache::new();
    let remote = Box::new(mock.clone()) as Box<dyn Cache>;
    let remote_check = Box::new(mock.clone()) as Box<dyn Cache>;

    let tiered = TieredCache::new(100, remote);
    tiered
        .set("key4", "value".as_bytes().to_vec(), None)
        .await
        .unwrap();

    tiered.del("key4").await.unwrap();

    // local 层（tiered.get 回退 remote 后仍 miss）+ remote 层均应已删。
    assert_eq!(tiered.get("key4").await.unwrap(), None);
    assert_eq!(remote_check.get("key4").await.unwrap(), None);
}

#[tokio::test]
async fn tiered_cache_miss_returns_none() {
    let _local = MemoryCache::new(100);
    let remote = Box::new(MockRemoteCache::new()) as Box<dyn Cache>;

    let tiered = TieredCache::new(100, remote);

    let result = tiered.get("nonexistent").await.unwrap();
    assert_eq!(result, None);
}
