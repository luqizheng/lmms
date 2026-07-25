//! GET /ntf/internal/migrations/state — notification-service schema 状态查询。
//!
//! Phase 10 G3 接入。复用 common-db::MigrationState trait。
//! 鉴权：service-jwt `aud=notification-service`（与 Phase 5-B 一致）。

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router, routing::get};
use common_db::migration_state::{self, MigrationDescriptor, MigrationState, MigrationStateResponse};
use serde_json::json;
use sqlx::MySqlPool;
use utoipa::path as to_path;

use crate::rest::RestState;

pub struct NotificationMigrationState;

impl MigrationState for NotificationMigrationState {
    fn service_name(&self) -> &'static str { "notification-service" }
    fn dialect(&self) -> &'static str { "mysql" }
    fn migrations(&self) -> &'static [MigrationDescriptor] {
        &[
            MigrationDescriptor { version: "V001", name: "init", marker_table: "ntf_announcements" },
            MigrationDescriptor { version: "V002", name: "ntf_tenant", marker_table: "ntf_migration_v002_applied" },
            MigrationDescriptor { version: "V003", name: "ntf_membership_projection", marker_table: "ntf_migration_v003_applied" },
        ]
    }
}

async fn probe_applied(pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    let state = NotificationMigrationState;
    let mut applied = Vec::with_capacity(state.migrations().len());
    for m in state.migrations() {
        if migration_state::probe_marker_mysql(pool, m.marker_table).await? {
            applied.push(m.version.to_string());
        }
    }
    Ok(applied)
}

#[to_path(
    get,
    path = "/ntf/internal/migrations/state",
    summary = "Get notification schema migration state",
    responses(
        (status = 200, body = MigrationStateResponse, description = "schema state"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 503, description = "db_unreachable"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal",
)]
pub async fn handler(State(state): State<RestState>) -> impl IntoResponse {
    let pool = state.db.pool.clone();
    let body: Result<MigrationStateResponse, (StatusCode, Json<serde_json::Value>)> =
        match probe_applied(&pool).await {
            Ok(applied) => Ok(migration_state::build_response(&NotificationMigrationState, applied)),
            Err(e) => Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "service": "notification-service",
                    "error": "db_unreachable",
                    "detail": e.to_string(),
                })),
            )),
        };
    match body {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err((s, j)) => (s, j).into_response(),
    }
}

pub fn routes(state: RestState) -> Router {
    Router::new()
        .route("/ntf/internal/migrations/state", get(handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_has_three_migrations() {
        let s = NotificationMigrationState;
        assert_eq!(s.service_name(), "notification-service");
        assert_eq!(s.dialect(), "mysql");
        assert_eq!(s.migrations().len(), 3);
        assert_eq!(s.migrations()[0].version, "V001");
        assert_eq!(s.migrations()[2].version, "V003");
        assert_eq!(s.migrations()[0].marker_table, "ntf_announcements");
    }
}