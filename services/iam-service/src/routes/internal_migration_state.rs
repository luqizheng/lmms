//! GET /iam/internal/migrations/state — schema 状态查询端点（Phase 10 G3）。
//!
//! 见 spec `docs/superpowers/specs/2026-07-08-migrations-state-endpoint-design.md`。
//!
//! 鉴权：service-jwt `aud=iam-service`（与 `iam/internal/*` 一致）。

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use digua_common_db::migration_state::{
    self, MigrationDescriptor, MigrationState, MigrationStateResponse,
};
use digua_common_db::DbPool;
use serde_json::json;
use utoipa::path as to_path;

use crate::state::AppState;

pub struct IamMigrationState;

impl MigrationState for IamMigrationState {
    fn service_name(&self) -> &'static str {
        "iam-service"
    }
    fn dialect(&self) -> &'static str {
        "mysql"
    }
    fn migrations(&self) -> &'static [MigrationDescriptor] {
        &[
            MigrationDescriptor {
                version: "V001",
                name: "init",
                marker_table: "iam_users",
            },
            MigrationDescriptor {
                version: "V002",
                name: "outbox_archive",
                marker_table: "iam_outbox_events_archive",
            },
            MigrationDescriptor {
                version: "V003",
                name: "tenants",
                marker_table: "iam_tenants",
            },
            MigrationDescriptor {
                version: "V004",
                name: "tenant_business_tables",
                marker_table: "iam_migration_v004_applied",
            },
            MigrationDescriptor {
                version: "V005",
                name: "platform_admin_role",
                marker_table: "iam_migration_v005_applied",
            },
            MigrationDescriptor {
                version: "V006",
                name: "org_unit_path",
                marker_table: "iam_migration_v006_applied",
            },
            MigrationDescriptor {
                version: "V007",
                name: "system_menu_permission",
                marker_table: "iam_migration_v007_applied",
            },
        ]
    }
}

async fn probe_applied(pool: &DbPool) -> Result<Vec<String>, sqlx::Error> {
    let state = IamMigrationState;
    let mut applied = Vec::with_capacity(state.migrations().len());
    for m in state.migrations() {
        // 方言无关探测入口（PG 用 current_schema()，MySQL 用 DATABASE()）。
        if migration_state::probe_marker(pool, m.marker_table)
            .await
            .map_err(|e| sqlx::Error::Configuration(Box::new(e)))?
        {
            applied.push(m.version.to_string());
        }
    }
    Ok(applied)
}

#[to_path(
    get,
    path = "/iam/internal/migrations/state",
    responses(
        (status = 200, body = MigrationStateResponse, description = "schema state"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 503, description = "db_unreachable"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal",
)]
pub async fn migration_state_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = state.db.clone();
    let body: Result<MigrationStateResponse, (StatusCode, Json<serde_json::Value>)> =
        match probe_applied(&pool).await {
            Ok(applied) => Ok(migration_state::build_response(&IamMigrationState, applied)),
            Err(e) => Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "service": "iam-service",
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

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/iam/internal/migrations/state",
            get(migration_state_handler),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iam_has_seven_migrations() {
        let s = IamMigrationState;
        assert_eq!(s.service_name(), "iam-service");
        assert_eq!(s.dialect(), "mysql");
        assert_eq!(s.migrations().len(), 7);
        assert_eq!(s.migrations()[0].version, "V001");
        assert_eq!(s.migrations()[6].version, "V007");
        assert_eq!(s.migrations()[0].marker_table, "iam_users");
        assert_eq!(s.migrations()[5].marker_table, "iam_migration_v006_applied");
        assert_eq!(s.migrations()[6].marker_table, "iam_migration_v007_applied");
    }
}
