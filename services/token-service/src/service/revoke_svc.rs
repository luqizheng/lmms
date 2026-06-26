use crate::db::DbPool;

pub async fn revoke(pool: &DbPool, jti: &str, ttl_sec: i64) -> Result<(), sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            sqlx::query(
                "INSERT IGNORE INTO token_revoked_jtis (jti, expires_at) \
                 VALUES (?, DATE_ADD(NOW(), INTERVAL ? SECOND))",
            )
            .bind(jti)
            .bind(ttl_sec)
            .execute(p)
            .await?;
            Ok(())
        }
    }
}

pub async fn is_revoked(pool: &DbPool, jti: &str) -> Result<bool, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT 1 FROM token_revoked_jtis WHERE jti = ? AND expires_at > NOW() LIMIT 1",
            )
            .bind(jti)
            .fetch_optional(p)
            .await?;
            Ok(row.is_some())
        }
    }
}
