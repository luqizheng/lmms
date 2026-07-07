use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

pub struct PermEpochCache {
    inner: RwLock<HashMap<u64, (Instant, Option<i64>)>>,
    ttl: Duration,
}

impl PermEpochCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, user_id: u64) -> (Option<i64>, bool) {
        if let Ok(map) = self.inner.read() {
            if let Some((ts, val)) = map.get(&user_id) {
                if ts.elapsed() < self.ttl {
                    return (*val, true);
                }
            }
        }
        (None, false)
    }

    pub fn set(&self, user_id: u64, epoch: Option<i64>) {
        if let Ok(mut map) = self.inner.write() {
            map.insert(user_id, (Instant::now(), epoch));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn cache_hit_returns_stored_value() {
        let cache = PermEpochCache::new(Duration::from_secs(5));
        cache.set(1, Some(100));
        let (epoch, hit) = cache.get(1);
        assert!(hit);
        assert_eq!(epoch, Some(100));
    }

    #[test]
    fn cache_miss_returns_false() {
        let cache = PermEpochCache::new(Duration::from_secs(5));
        let (epoch, hit) = cache.get(999);
        assert!(!hit);
        assert_eq!(epoch, None);
    }

    #[test]
    fn cache_expires_after_ttl() {
        let cache = PermEpochCache::new(Duration::from_millis(1));
        cache.set(1, Some(100));
        thread::sleep(Duration::from_millis(2));
        let (_, hit) = cache.get(1);
        assert!(!hit);
    }

    #[test]
    fn cache_stores_none() {
        let cache = PermEpochCache::new(Duration::from_secs(5));
        cache.set(1, None);
        let (epoch, hit) = cache.get(1);
        assert!(hit);
        assert_eq!(epoch, None);
    }
}
