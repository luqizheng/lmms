//! Shared helper for writing rows into `iam_user_roles` inside a caller's
//! business transaction. Used by both `UserService::create` (initial role
//! assignment at user creation) and `RoleService::assign_user_roles`
//! (Phase 4.4-fu1 full-replacement endpoint).
//!
//! Empty `role_ids` → no-op. Duplicates in `role_ids` are the caller's
//! responsibility to dedup; a duplicate would violate the `(user_id, role_id)`
//! primary key and abort the transaction.

use sqlx::{MySql, Transaction};

use crate::error::IamError;

pub(crate) async fn set_user_roles_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: &str,
    role_ids: &[String],
) -> Result<(), IamError> {
    for role_id in role_ids {
        sqlx::query(
            "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())",
        )
        .bind(user_id)
        .bind(role_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
