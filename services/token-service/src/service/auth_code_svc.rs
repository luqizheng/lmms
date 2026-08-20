use crate::db::DbPool;

/// 写入新授权码，TTL 单位秒。重复 code 触发 1062 → 视为已存在（no-op）。
pub async fn save(
    pool: &DbPool,
    code: &str,
    user_id: &str,
    ttl_sec: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let res = sqlx::query(
                "INSERT INTO token_auth_codes (code, user_id, expires_at) \
                 VALUES (?, ?, DATE_ADD(NOW(), INTERVAL ? SECOND))",
            )
            .bind(code)
            .bind(user_id)
            .bind(ttl_sec)
            .execute(p)
            .await;
            match res {
                Ok(_) => Ok(()),
                Err(sqlx::Error::Database(ref d)) if d.code().as_deref() == Some("23000") => {
                    // 重复 PK 视为 idempotent no-op
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// 原子两步 consume：
///   1. UPDATE ... SET consumed_at = NOW() WHERE 未消费 + 未过期
///   2. SELECT user_id
///
/// 返回 Some(user_id) 当且仅当第一步成功。
pub async fn consume(pool: &DbPool, code: &str) -> Result<Option<String>, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let res = sqlx::query(
                "UPDATE token_auth_codes SET consumed_at = NOW() \
                 WHERE code = ? AND consumed_at IS NULL AND expires_at > NOW()",
            )
            .bind(code)
            .execute(p)
            .await?;
            if res.rows_affected() == 0 {
                return Ok(None);
            }
            let row: Option<(String,)> =
                sqlx::query_as("SELECT user_id FROM token_auth_codes WHERE code = ?")
                    .bind(code)
                    .fetch_optional(p)
                    .await?;
            Ok(row.map(|(u,)| u))
        }
    }
}
