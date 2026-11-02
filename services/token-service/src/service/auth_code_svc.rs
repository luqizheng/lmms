use crate::db::DbPool;
use digua_common_db::{db_execute, db_fetch_optional};

/// 写入新授权码，TTL 单位秒。重复 code 视为已存在（幂等 no-op）。
///
/// `expires_at` 由**应用侧**算出（`Utc::now() + ttl`）后绑定，
/// 不再使用 MySQL 专有的 `DATE_ADD(NOW(), INTERVAL ? SECOND)`——
/// 一条 SQL 同时适用于 MySQL / PostgreSQL。两侧会话时区都锁 UTC，
/// 因此应用时钟与数据库时钟对 `TIMESTAMP` / `TIMESTAMPTZ` 的解释一致。
pub async fn save(
    pool: &DbPool,
    code: &str,
    user_id: &str,
    ttl_sec: i64,
) -> Result<(), sqlx::Error> {
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl_sec);
    let res = db_execute!(
        pool,
        "INSERT INTO token_auth_codes (code, user_id, expires_at) VALUES (?, ?, ?)",
        code,
        user_id,
        expires_at
    );
    match res {
        Ok(_) => Ok(()),
        // 重复 PK 视为 idempotent no-op（MySQL 23000 / PG 23505）。
        Err(ref e) if crate::db::is_duplicate_key(e) => Ok(()),
        Err(e) => Err(e),
    }
}

/// 原子两步 consume：
///   1. UPDATE ... SET consumed_at = NOW() WHERE 未消费 + 未过期
///   2. SELECT user_id
///
/// 返回 Some(user_id) 当且仅当第一步成功。
pub async fn consume(pool: &DbPool, code: &str) -> Result<Option<String>, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "UPDATE token_auth_codes SET consumed_at = NOW() \
         WHERE code = ? AND consumed_at IS NULL AND expires_at > NOW()",
        code
    )?;
    if affected == 0 {
        return Ok(None);
    }
    let row: Option<(String,)> = db_fetch_optional!(
        pool,
        (String,),
        "SELECT user_id FROM token_auth_codes WHERE code = ?",
        code
    )?;
    Ok(row.map(|(u,)| u))
}
