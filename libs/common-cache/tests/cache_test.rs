use common_cache::{Cache, MemoryCache};

#[tokio::test]
async fn memory_cache_set_get_delete() {
    let c = MemoryCache::new(100);
    c.set("key1", "val1".as_bytes().to_vec(), None).await.unwrap();
    assert_eq!(c.get("key1").await.unwrap(), Some("val1".as_bytes().to_vec()));
    c.del("key1").await.unwrap();
    assert_eq!(c.get("key1").await.unwrap(), None);
}

#[tokio::test]
async fn memory_cache_overwrite() {
    let c = MemoryCache::new(100);
    c.set("key", "v1".as_bytes().to_vec(), None).await.unwrap();
    c.set("key", "v2".as_bytes().to_vec(), None).await.unwrap();
    assert_eq!(c.get("key").await.unwrap(), Some("v2".as_bytes().to_vec()));
}

#[tokio::test]
async fn memory_cache_empty_key() {
    let c = MemoryCache::new(100);
    assert_eq!(c.get("").await.unwrap(), None);
    c.set("", "val".as_bytes().to_vec(), None).await.unwrap();
    assert_eq!(c.get("").await.unwrap(), Some("val".as_bytes().to_vec()));
}