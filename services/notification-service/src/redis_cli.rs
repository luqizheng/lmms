use crate::error::AppResult;
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

    pub fn unread_key(user_id: String) -> String {
        format!("notif:unread:{user_id}")
    }

    pub async fn incr_unread(&self, user_id: String, delta: i64) -> AppResult<()> {
        let mut c = self.conn.clone();
        let _: i64 = c.incr(Self::unread_key(user_id), delta).await?;
        Ok(())
    }

    pub async fn decr_unread_floor0(&self, user_id: String) -> AppResult<()> {
        let mut c = self.conn.clone();
        let key = Self::unread_key(user_id);
        let v: i64 = c.decr(&key, 1).await?;
        if v < 0 {
            let _: () = c.set(&key, 0).await?;
        }
        Ok(())
    }

    pub async fn get_unread(&self, user_id: String) -> AppResult<i64> {
        let mut c = self.conn.clone();
        let v: Option<i64> = c.get(Self::unread_key(user_id)).await?;
        Ok(v.unwrap_or(0))
    }
}
