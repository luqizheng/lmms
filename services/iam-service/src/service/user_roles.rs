//! Shared helper for writing rows into `iam_user_roles` inside a caller's
//! business transaction. Used by both `UserService::create` (initial role
//! assignment at user creation) and `RoleService::assign_user_roles`
//! (Phase 4.4-fu1 full-replacement endpoint).
//!
//! Empty `role_ids` → no-op. Duplicates in `role_ids` are the caller's
//! responsibility to dedup; a duplicate would violate the `(user_id, role_id)`
//! primary key and abort the transaction.

use digua_common_event::AnyTx;

use crate::error::IamError;

/// 事务参数是 `AnyTx`：调用方在方言臂内包装。写入 SQL 按方言臂执行
/// （`db_*!` 宏只吃 `DbPool`，不吃事务连接）。
pub(crate) async fn set_user_roles_in_tx(
    tx: &mut AnyTx<'_, '_>,
    user_id: &str,
    role_ids: &[String],
) -> Result<(), IamError> {
    match tx {
        #[cfg(feature = "mysql")]
        AnyTx::Mysql(_tx) => {
            for role_id in role_ids {
                sqlx::query(
                    "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())",
                )
                .bind(user_id)
                .bind(role_id)
                // match ergonomics：`_tx` 是 `&mut &'a mut Transaction`，execute
                // 要 `&mut Connection` → 三层解引用。
                .execute(&mut ***_tx)
                .await?;
            }
        }
        // PG 臂：$N 占位符；NOW() → CURRENT_TIMESTAMP(3)（归属表 #21/#22）。
        #[cfg(feature = "postgres")]
        AnyTx::Postgres(_tx) => {
            for role_id in role_ids {
                sqlx::query(
                    "INSERT INTO iam_user_roles (user_id, role_id, created_at) \
                     VALUES ($1, $2, CURRENT_TIMESTAMP(3))",
                )
                .bind(user_id)
                .bind(role_id)
                // match ergonomics：`_tx` 是 `&mut &'a mut Transaction`，execute
                // 要 `&mut Connection` → 三层解引用。
                .execute(&mut ***_tx)
                .await?;
            }
        }
        #[allow(unreachable_patterns)]
        _ => {
            return Err(IamError::Internal(
                "set_user_roles_in_tx: dialect feature not enabled".into(),
            ))
        }
    }
    Ok(())
}
