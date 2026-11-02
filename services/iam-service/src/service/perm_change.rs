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

use crate::error::IamError;
use crate::state::AppState;

/// Enqueue the `iam.permission.changed` event inside the caller's business
/// transaction (atomic with the DB mutation). Empty `user_ids` → skip, no
/// enqueue.
///
/// 事务参数是 `AnyTx`：tenant 解析 SQL 按方言臂执行（事务连接是具体类型，
/// `db_*!` 宏只吃 `DbPool`）；PG 侧 `iam_users.tenant_id` 为有符号 BIGINT。
pub async fn enqueue_permission_changed(
    tx: &mut AnyTx<'_, '_>,
    repo: &MySqlOutboxRepo,
    user_ids: &[String],
    reason: &str,
    actor_id: Option<&str>,
) -> Result<(), IamError> {
    if user_ids.is_empty() {
        return Ok(());
    }
    let distinct_tids: Vec<i64> = match tx {
        #[cfg(feature = "mysql")]
        AnyTx::Mysql(tx) => resolve_distinct_tids_mysql(tx, user_ids).await?,
        #[cfg(feature = "postgres")]
        AnyTx::Postgres(tx) => resolve_distinct_tids_pg(tx, user_ids).await?,
        #[allow(unreachable_patterns)]
        _ => {
            return Err(IamError::Internal(
                "perm_change: dialect feature not enabled".into(),
            ))
        }
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
    repo.enqueue(tx, &mut env)
        .await
        .map_err(|e| IamError::Internal(e.to_string()))?;
    Ok(())
}

/// MySQL 臂：V009 起 tenant_id 已是有符号 BIGINT（与 PG 一致），直接 i64。
#[cfg(feature = "mysql")]
async fn resolve_distinct_tids_mysql(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user_ids: &[String],
) -> Result<Vec<i64>, IamError> {
    if user_ids.len() == 1 {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT tenant_id FROM iam_users WHERE id = ? AND deleted_at IS NULL")
                .bind(&user_ids[0])
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| IamError::Internal(e.to_string()))?;
        Ok(row.map(|(t,)| t).into_iter().collect())
    } else {
        let placeholders: Vec<String> =
            std::iter::repeat_n("?".to_string(), user_ids.len()).collect();
        let sql = format!(
            "SELECT DISTINCT tenant_id FROM iam_users WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders.join(",")
        );
        let mut q = sqlx::query_as::<_, (i64,)>(&sql);
        for uid in user_ids {
            q = q.bind(uid);
        }
        let rows: Vec<(i64,)> = q
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| IamError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(|(t,)| t).collect())
    }
}

/// PG 臂：占位符必须 `$N` 编号式（含动态 IN 列表）；tenant_id 有符号。
#[cfg(feature = "postgres")]
async fn resolve_distinct_tids_pg(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_ids: &[String],
) -> Result<Vec<i64>, IamError> {
    if user_ids.len() == 1 {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT tenant_id FROM iam_users WHERE id = $1 AND deleted_at IS NULL")
                .bind(&user_ids[0])
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| IamError::Internal(e.to_string()))?;
        Ok(row.map(|(t,)| t).into_iter().collect())
    } else {
        let placeholders: Vec<String> = (1..=user_ids.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "SELECT DISTINCT tenant_id FROM iam_users WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders.join(",")
        );
        let mut q = sqlx::query_as::<_, (i64,)>(&sql);
        for uid in user_ids {
            q = q.bind(uid);
        }
        let rows: Vec<(i64,)> = q
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| IamError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(|(t,)| t).collect())
    }
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
