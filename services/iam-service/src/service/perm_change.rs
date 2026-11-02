//! Shared perm-change publish + direct-write routines (Phase 4.4).
//!
//! On every permission-changing write iam must, in the SAME business
//! transaction, enqueue an `iam.permission.changed` outbox event; then,
//! AFTER commit, best-effort stamp each affected user's Redis perm_epoch so
//! the gateway invalidates JWTs issued before the change. The outbox event is
//! the durable path (gateway consumer backfills); the direct write is a
//! latency optimization only — failures are warned, never propagated.

use digua_common_event::events::IamPermissionChanged;
use digua_common_event::perm_epoch::set_perm_epoch_if_greater;
use digua_common_event::{AnyTx, Envelope, MySqlOutboxRepo, OutboxRepo};
use sqlx::{MySql, Transaction};

use crate::error::IamError;
use crate::state::AppState;

/// Enqueue the `iam.permission.changed` event inside the caller's business
/// transaction (atomic with the DB mutation). Empty `user_ids` → skip, no
/// enqueue.
pub async fn enqueue_permission_changed(
    tx: &mut Transaction<'_, MySql>,
    repo: &MySqlOutboxRepo,
    user_ids: &[String],
    reason: &str,
    actor_id: Option<&str>,
) -> Result<(), IamError> {
    if user_ids.is_empty() {
        return Ok(());
    }
    let distinct_tids: Vec<i64> = if user_ids.len() == 1 {
        let row: Option<(u64,)> =
            sqlx::query_as("SELECT tenant_id FROM iam_users WHERE id = ? AND deleted_at IS NULL")
                .bind(&user_ids[0])
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| IamError::Internal(e.to_string()))?;
        row.map(|(t,)| t as i64).into_iter().collect()
    } else {
        let placeholders: Vec<String> =
            std::iter::repeat_n("?".to_string(), user_ids.len()).collect();
        let sql = format!(
            "SELECT DISTINCT tenant_id FROM iam_users WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders.join(",")
        );
        let mut q = sqlx::query_as::<_, (u64,)>(&sql);
        for uid in user_ids {
            q = q.bind(uid);
        }
        let rows: Vec<(u64,)> = q
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| IamError::Internal(e.to_string()))?;
        rows.into_iter().map(|(t,)| t as i64).collect()
    };
    debug_assert!(
        distinct_tids.len() <= 1,
        "perm_change across tenants: users={:?}",
        user_ids
    );
    if distinct_tids.len() > 1 {
        tracing::warn!(users = ?user_ids, tids = ?distinct_tids, "perm_change event spans multiple tenants; using first tenant for envelope");
    }
    let tenant_id = distinct_tids.first().copied().unwrap_or(1);
    let mut env = Envelope::new(
        "iam-service",
        IamPermissionChanged {
            user_ids: serde_json::json!(user_ids),
            reason: reason.to_string(),
            actor_id: actor_id.map(|s| s.to_string()),
        },
    )
    .with_tenant_id(tenant_id.to_string());
    repo.enqueue(&mut AnyTx::Mysql(tx), &mut env)
        .await
        .map_err(|e| IamError::Internal(e.to_string()))?;
    Ok(())
}

/// Best-effort direct write of each user's `perm_epoch` AFTER the business tx
/// commits. Redis errors are warned only (gateway consumer backfills via the
/// outbox event); a `None` redis handle is a no-op.
pub async fn stamp_perm_epochs(state: &AppState, user_ids: &[String]) {
    let Some(conn) = state.redis.as_ref() else {
        return;
    };
    let now = chrono::Utc::now().timestamp();
    let mut conn = conn.clone();
    for uid in user_ids {
        if let Err(e) = set_perm_epoch_if_greater(
            &mut conn,
            &state.redis_prefix,
            uid,
            now,
            state.perm_epoch_ttl_sec,
        )
        .await
        {
            tracing::warn!(error=%e, uid, "iam direct perm_epoch write failed (gateway consumer will backfill)");
        }
    }
}
