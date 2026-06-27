use redis::aio::ConnectionManager;
use redis::AsyncCommands;

#[derive(Clone)]
pub struct RedisCli {
    pub conn: ConnectionManager,
}

impl RedisCli {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { conn })
    }

    /// INCR key; on first hit (==1) set EXPIRE window_secs. Returns the new count.
    /// Sliding-window semantics: the key lives for window_secs from its first hit.
    pub async fn incr_with_window(&self, key: &str, window_secs: usize) -> anyhow::Result<i64> {
        let mut c = self.conn.clone();
        let n: i64 = c.incr(key, 1).await?;
        if n == 1 {
            let _: () = c.expire(key, window_secs as i64).await?;
        }
        Ok(n)
    }
}
