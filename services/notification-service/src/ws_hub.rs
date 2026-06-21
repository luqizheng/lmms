use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsOut {
    #[serde(rename = "new")]
    New(serde_json::Value),
    #[serde(rename = "unread_count")]
    UnreadCount { count: i64 },
    #[serde(rename = "revoked")]
    Revoked { id: u64 },
    #[serde(rename = "pong")]
    Pong,
}

#[derive(Clone, Default)]
pub struct WsHub {
    inner: Arc<RwLock<HubInner>>,
}

#[derive(Default)]
struct HubInner {
    channels: HashMap<String, broadcast::Sender<String>>,
}

impl WsHub {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HubInner {
                channels: HashMap::new(),
            })),
        }
    }

    async fn channel(&self, user_id: String) -> broadcast::Sender<String> {
        if let Some(t) = self.inner.read().await.channels.get(&user_id).cloned() {
            return t;
        }
        let mut w = self.inner.write().await;
        w.channels
            .entry(user_id)
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(64);
                tx
            })
            .clone()
    }

    pub async fn subscribe(&self, user_id: String) -> broadcast::Receiver<String> {
        self.channel(user_id).await.subscribe()
    }

    pub async fn publish(&self, user_id: String, out: WsOut) {
        let payload = serde_json::to_string(&out).unwrap_or_default();
        let tx = self.channel(user_id).await;
        let _ = tx.send(payload);
    }
}
