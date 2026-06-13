use common_cache::{Cache, MemoryCache};
use std::time::Duration;

#[tokio::test]
async fn memory_set_get_roundtrip() {
    let c = MemoryCache::new(64);
    c.set("k", b"v".to_vec(), Some(Duration::from_secs(60))).await.unwrap();
    let got = c.get("k").await.unwrap();
    assert_eq!(got.as_deref(), Some(&b"v"[..]));
}

#[tokio::test]
async fn memory_del_removes_key() {
    let c = MemoryCache::new(64);
    c.set("k", b"v".to_vec(), None).await.unwrap();
    c.del("k").await.unwrap();
    assert!(c.get("k").await.unwrap().is_none());
}