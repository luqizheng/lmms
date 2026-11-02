use crate::db::DbPool;
use digua_common_db::{db_execute, db_fetch_scalar};

/// 记录一个 refresh jti（幂等：重复 jti 视为已记录）。
///
/// 跨方言语义：MySQL 原来是 `INSERT IGNORE`，PG 没有等价语法，
/// 故统一写裸 `INSERT` 并把「重复键」错误视为成功（见 `db::is_duplicate_key`）。
pub async fn mark(pool: &DbPool, jti: &str, ttl_sec: i64) -> Result<(), sqlx::Error> {
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl_sec);
    let res = db_execute!(
        pool,
        "INSERT INTO token_refresh_markers (jti, expires_at) VALUES (?, ?)",
        jti,
        expires_at
    );
    match res {
        Ok(_) => Ok(()),
        Err(ref e) if crate::db::is_duplicate_key(e) => Ok(()),
        Err(e) => Err(e),
    }
}

pub async fn is_marked(pool: &DbPool, jti: &str) -> Result<bool, sqlx::Error> {
    // 用 COUNT(*) 而不是 `SELECT 1`：`SELECT 1` 在 MySQL 返回 INT、在 PG 返回 int4，
    // 两方言解到 i64 的兼容性不一致；COUNT(*) 两侧都是 BIGINT。
    let n: i64 = db_fetch_scalar!(
        pool,
        i64,
        "SELECT COUNT(*) FROM token_refresh_markers WHERE jti = ? AND expires_at > NOW()",
        jti
    )?;
    Ok(n > 0)
}
