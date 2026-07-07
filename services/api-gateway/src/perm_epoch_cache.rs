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
