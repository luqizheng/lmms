//! GET /prue/internal/migrations/state — prue_backend schema 状态查询。
//!
//! Phase 10 G3 接入。prue_backend 是双 dialect 服务（MySQL + SQLite）。
//! 1 migration × 2 dialects：V001 init (marker: prue_menu_configs)。

use axum::{
    extract::Extension, http::StatusCode, response::IntoResponse, routing::get, Json, Router,
};
use digua_common_db::migration_state::MigrationStateResponse;
use digua_common_db::{
    migration_state::{self, MigrationDescriptor, MigrationState},
    DbPool,
};
use serde_json::json;
use std::sync::Arc;
use utoipa::path as to_path;

pub struct PrueMigrationState;

impl MigrationState for PrueMigrationState {
    fn service_name(&self) -> &'static str {
        "prue_backend"
    }
    fn dialect(&self) -> &'static str {
        "mysql+sqlite"
    }
    fn migrations(&self) -> &'static [MigrationDescriptor] {
        &[MigrationDescriptor {
            version: "V001",
            name: "init",
            marker_table: "prue_menu_configs",
        }]
    }
}

async fn probe_applied(pool: &DbPool) -> Result<Vec<String>, sqlx::Error> {
    let state = PrueMigrationState;
    let mut applied = Vec::with_capacity(state.migrations().len());
    for m in state.migrations() {
        let exists = match pool {
            DbPool::Mysql(p) => migration_state::probe_marker_mysql(p, m.marker_table).await?,
            DbPool::Sqlite(p) => migration_state::probe_marker_sqlite(p, m.marker_table).await?,
        };
        if exists {
            applied.push(m.version.to_string());
        }
    }
    Ok(applied)
}

#[to_path(
    get,
    path = "/prue/internal/migrations/state",
    summary = "Query prue_backend migration state (mysql+sqlite)",
    responses(
        (status = 200, body = MigrationStateResponse, description = "schema state"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 503, description = "db_unreachable"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal",
)]
pub async fn handler(Extension(pool): Extension<Arc<DbPool>>) -> impl IntoResponse {
    match probe_applied(&pool).await {
        Ok(applied) => {
            let resp = migration_state::build_response(&PrueMigrationState, applied);
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "service": "prue_backend",
                "error": "db_unreachable",
                "detail": e.to_string(),
            })),
        )
            .into_response(),
    }
}

pub fn routes(state: crate::state::AppState) -> Router {
    Router::new()
        .route("/prue/internal/migrations/state", get(handler))
        .layer(Extension(state.db.clone()))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prue_has_one_migration_dual_dialect() {
        let s = PrueMigrationState;
        assert_eq!(s.service_name(), "prue_backend");
        assert_eq!(s.dialect(), "mysql+sqlite");
        assert_eq!(s.migrations().len(), 1);
        assert_eq!(s.migrations()[0].version, "V001");
        assert_eq!(s.migrations()[0].marker_table, "prue_menu_configs");
    }
}
