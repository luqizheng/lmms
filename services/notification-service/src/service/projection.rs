//! Phase 5B-fu3: notification → iam membership bootstrap routine.
//!
//! Called once at notification-service startup (in main, before spawning the
//! membership consumer). Fetches the full user→{role_ids, dept_id} map from
//! iam's `/iam/internal/membership-bootstrap` endpoint and replaces the local
//! `ntf_user_projection` table content.
//!
//! ## Ordering with consumer
//!
//! Bootstrap → consumer start → consumer handles events from now on.
//! Events emitted during bootstrap are NOT replayed (no outbox_archive
//! replay); `last_event_at` in the projection is set to 0 so any subsequent
//! event with `ts > 0` applies. This gives eventual consistency — the
//! bootstrap is the "snapshot" and the consumer is the "delta stream".
//!
//! ## Failure modes
//!
//! - iam unreachable → fail-fast at startup (readiness probe will also
//!   reflect this)
//! - DB error mid-bootstrap → tx aborts, prior projection rows preserved

use crate::db::Db;
use crate::error::AppResult;
use crate::iam_client::IamMemberClient;

pub async fn bootstrap(db: &Db, iam_client: &IamMemberClient) -> AppResult<()> {
    let snap = iam_client.fetch_membership_bootstrap().await?;
    let mappings = snap
        .mappings
        .into_iter()
        .map(|(uid, row)| {
            (
                uid,
                crate::models::UserMembership {
                    role_ids: row.role_ids,
                    dept_id: row.dept_id,
                },
            )
        })
        .collect();
    db.replace_projection(snap.tenant_id, &mappings).await
}
