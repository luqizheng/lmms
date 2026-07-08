//! GET /cfg/internal/migrations/state — config-service schema 状态查询。

use axum::{extract::Extension, http::StatusCode, response::IntoResponse, Json, Router, routing::get};
use common_db::migration_state::{self, MigrationDescriptor, MigrationState, MigrationStateResponse};
use serde_json::json;
use sqlx::MySqlPool;
use utoipa::path as to_path;

use crate::services::ConfigService;

pub struct ConfigMigrationState;

impl MigrationState for ConfigMigrationState {
    fn service_name(&self) -> &'static str { "config-service" }
    fn dialect(&self) -> &'static str { "mysql" }
    fn migrations(&self) -> &'static [MigrationDescriptor] {
        &[
            MigrationDescriptor { version: "V001", name: "init", marker_table: "cfg_namespaces" },
            MigrationDescriptor { version: "V002", name: "tenant_cfg", marker_table: "cfg_migration_v002_applied" },
        ]
    }
}

async fn probe_applied(pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    let state = ConfigMigrationState;
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
    path = "/cfg/internal/migrations/state",
    responses(
        (status = 200, body = MigrationStateResponse, description = "schema state"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 503, description = "db_unreachable"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal",
)]
pub async fn handler(Extension(pool): Extension<MySqlPool>) -> impl IntoResponse {
    match probe_applied(&pool).await {
        Ok(applied) => {
            let resp = migration_state::build_response(&ConfigMigrationState, applied);
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "service": "config-service",
                "error": "db_unreachable",
                "detail": e.to_string(),
            })),
        ).into_response(),
    }
}

pub fn routes(state: ConfigService) -> Router<ConfigService> {
    Router::new()
        .route("/cfg/internal/migrations/state", get(handler))
        .layer(Extension(state.pool_ref().clone()))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_has_two_migrations() {
        let s = ConfigMigrationState;
        assert_eq!(s.service_name(), "config-service");
        assert_eq!(s.dialect(), "mysql");
        assert_eq!(s.migrations().len(), 2);
        assert_eq!(s.migrations()[0].version, "V001");
        assert_eq!(s.migrations()[1].version, "V002");
        assert_eq!(s.migrations()[0].marker_table, "cfg_namespaces");
        assert_eq!(s.migrations()[1].marker_table, "cfg_migration_v002_applied");
    }
}