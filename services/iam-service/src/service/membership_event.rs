//! Membership event emit helpers (Phase 5B-fu3).
//!
//! Tx-in outbox emit following the `enqueue_permission_changed` pattern
//! (`service/perm_change.rs`). All helpers take `&mut Transaction` so the
//! emit is atomic with the caller's SQL mutation. `ts` is server-side
//! Unix seconds — consumers compare against `last_event_at` to drop
//! out-of-order events (see spec §3.6 D6).

use digua_common_event::events::{
    IamUserDeptChanged, IamUserRoleAssigned, IamUserRoleUnassigned,
};
use digua_common_event::{Envelope, MySqlOutboxRepo, OutboxRepo};
use sqlx::{MySql, Transaction};

use crate::error::IamError;

pub async fn enqueue_role_assigned(
    tx: &mut Transaction<'_, MySql>,
    repo: &MySqlOutboxRepo,
    user_id: &str,
    role_id: &str,
    tenant_id: i64,
    actor_id: Option<&str>,
    ts: i64,
) -> Result<(), IamError> {
    let mut env = Envelope::new(
        "iam-service",
        IamUserRoleAssigned {
            user_id: user_id.to_string(),
            role_id: role_id.to_string(),
            tenant_id,
            actor_id: actor_id.map(String::from),
            ts,
        },
    )
    .with_tenant_id(tenant_id.to_string());
    repo.enqueue(tx, &mut env)
        .await
        .map_err(|e| IamError::Internal(e.to_string()))
}

pub async fn enqueue_role_unassigned(
    tx: &mut Transaction<'_, MySql>,
    repo: &MySqlOutboxRepo,
    user_id: &str,
    role_id: &str,
    tenant_id: i64,
    actor_id: Option<&str>,
    ts: i64,
) -> Result<(), IamError> {
    let mut env = Envelope::new(
        "iam-service",
        IamUserRoleUnassigned {
            user_id: user_id.to_string(),
            role_id: role_id.to_string(),
            tenant_id,
            actor_id: actor_id.map(String::from),
            ts,
        },
    )
    .with_tenant_id(tenant_id.to_string());
    repo.enqueue(tx, &mut env)
        .await
        .map_err(|e| IamError::Internal(e.to_string()))
}

pub async fn enqueue_dept_changed(
    tx: &mut Transaction<'_, MySql>,
    repo: &MySqlOutboxRepo,
    user_id: &str,
    dept_id: Option<&str>,
    tenant_id: i64,
    actor_id: Option<&str>,
    ts: i64,
) -> Result<(), IamError> {
    let mut env = Envelope::new(
        "iam-service",
        IamUserDeptChanged {
            user_id: user_id.to_string(),
            dept_id: dept_id.map(String::from),
            tenant_id,
            actor_id: actor_id.map(String::from),
            ts,
        },
    )
    .with_tenant_id(tenant_id.to_string());
    repo.enqueue(tx, &mut env)
        .await
        .map_err(|e| IamError::Internal(e.to_string()))
}