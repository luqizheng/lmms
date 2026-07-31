//! GET /audit/internal/migrations/state — audit-service schema 状态查询。
//!
//! Phase 10 G3 接入。

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router, routing::get};
use digua_common_db::migration_state::{self, MigrationDescriptor, MigrationState, MigrationStateResponse};
use serde_json::json;
use utoipa::path as to_path;

use super::AppServices;

pub struct AuditMigrationState;

impl MigrationState for AuditMigrationState {
    fn service_name(&self) -> &'static str { "audit-service" }
    fn dialect(&self) -> &'static str { "mysql" }
    fn migrations(&self) -> &'static [MigrationDescriptor] {
        &[MigrationDescriptor { version: "V001", name: "init", marker_table: "audit_operation_logs" }]
    }
}

#[to_path(
    get,
    path = "/audit/internal/migrations/state",
    summary = "Get audit migration state",
    responses(
        (status = 200, body = MigrationStateResponse, description = "schema state"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 503, description = "db_unreachable"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal",
)]
pub async fn handler(State(state): State<super::AppState>) -> impl IntoResponse {
    let pool = state.audit.pool();
    let body: Result<MigrationStateResponse, (StatusCode, Json<serde_json::Value>)> =
        match migration_state::probe_marker_mysql(pool, "audit_operation_logs").await {
            Ok(true) => Ok(migration_state::build_response(&AuditMigrationState, vec!["V001".into()])),
            Ok(false) => Ok(migration_state::build_response(&AuditMigrationState, vec![])),
            Err(e) => Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "service": "audit-service",
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

pub fn routes(state: super::AppState) -> Router {
    Router::new()
        .route("/audit/internal/migrations/state", get(handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_has_one_migration() {
        let s = AuditMigrationState;
        assert_eq!(s.service_name(), "audit-service");
        assert_eq!(s.dialect(), "mysql");
        assert_eq!(s.migrations().len(), 1);
        assert_eq!(s.migrations()[0].version, "V001");
        assert_eq!(s.migrations()[0].marker_table, "audit_operation_logs");
    }
}