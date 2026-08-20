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
    let local = MemoryCache::new(100);
    let remote = Box::new(MockRemoteCache::new()) as Box<dyn Cache>;

    local
        .set("key1", "local_value".as_bytes().to_vec(), None)
        .await
        .unwrap();

    let tiered = TieredCache::new(100, remote);

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
    let local = MemoryCache::new(100);
    let mock = MockRemoteCache::new();
    let remote = Box::new(mock.clone()) as Box<dyn Cache>;
    let remote_check = Box::new(mock.clone()) as Box<dyn Cache>;

    let tiered = TieredCache::new(100, remote);

    tiered
        .set("key3", "both_value".as_bytes().to_vec(), None)
        .await
        .unwrap();

    assert_eq!(
        local.get("key3").await.unwrap(),
        Some("both_value".as_bytes().to_vec())
    );
    assert_eq!(
        remote_check.get("key3").await.unwrap(),
        Some("both_value".as_bytes().to_vec())
    );
}

#[tokio::test]
async fn tiered_cache_del_propagates_to_both() {
    let local = MemoryCache::new(100);
    let mock = MockRemoteCache::new();
    let remote = Box::new(mock.clone()) as Box<dyn Cache>;
    let remote_check = Box::new(mock.clone()) as Box<dyn Cache>;

    local
        .set("key4", "value".as_bytes().to_vec(), None)
        .await
        .unwrap();
    remote_check
        .set("key4", "value".as_bytes().to_vec(), None)
        .await
        .unwrap();

    let tiered = TieredCache::new(100, remote);
    tiered.del("key4").await.unwrap();

    assert_eq!(local.get("key4").await.unwrap(), None);
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
