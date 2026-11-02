pub use digua_common_db::dialect::DialectKind;
pub use digua_common_db::DbPool;

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

/// 连接 MySQL 池（历史 API，签名与行为保持不变）。
///
/// 新代码请用 [`connect_pool`]——它按 URL 判定方言，返回可跨方言的 [`DbPool`]。
pub async fn connect(url: &str, max: u32) -> anyhow::Result<MySqlPool> {
    Ok(MySqlPoolOptions::new()
        .max_connections(max)
        .acquire_timeout(Duration::from_secs(5))
        .connect(url)
        .await?)
}

/// 连接**方言感知**的连接池：按 URL 判定后端，返回 [`DbPool`]。
///
/// PostgreSQL 分支会锁定会话时区为 UTC（与基座层 `now_expr()` 的 UTC 语义对齐）。
pub async fn connect_pool(url: &str, max: u32) -> anyhow::Result<DbPool> {
    Ok(digua_common_db::init_pool_with(url, max).await?)
}
