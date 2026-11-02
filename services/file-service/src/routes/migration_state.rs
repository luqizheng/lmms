//! GET /file/internal/migrations/state — file-service schema 状态查询。

use axum::{
    extract::Extension, http::StatusCode, response::IntoResponse, routing::get, Json, Router,
};
use digua_common_db::migration_state::{
    self, MigrationDescriptor, MigrationState, MigrationStateResponse,
};
use serde_json::json;
use digua_common_db::DbPool;
use utoipa::path as to_path;

use crate::services::FileService;

pub struct FileMigrationState;

impl MigrationState for FileMigrationState {
    fn service_name(&self) -> &'static str {
        "file-service"
    }
    fn dialect(&self) -> &'static str {
        "mysql"
    }
    fn migrations(&self) -> &'static [MigrationDescriptor] {
        &[
            MigrationDescriptor {
                version: "V001",
                name: "init",
                marker_table: "file_files",
            },
            MigrationDescriptor {
                version: "V002",
                name: "file_tenant_reconcile",
                marker_table: "file_migration_v002_applied",
            },
        ]
    }
}

async fn probe_applied(pool: &DbPool) -> Result<Vec<String>, sqlx::Error> {
    let state = FileMigrationState;
    let mut applied = Vec::with_capacity(state.migrations().len());
    for m in state.migrations() {
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
    path = "/file/internal/migrations/state",
    summary = "Get file-service schema migration state",
    responses(
        (status = 200, body = MigrationStateResponse, description = "schema state"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 503, description = "db_unreachable"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal",
)]
pub async fn handler(Extension(pool): Extension<DbPool>) -> impl IntoResponse {
    match probe_applied(&pool).await {
        Ok(applied) => {
            let resp = migration_state::build_response(&FileMigrationState, applied);
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "service": "file-service",
                "error": "db_unreachable",
                "detail": e.to_string(),
            })),
        )
            .into_response(),
    }
}

pub fn routes(state: FileService) -> Router<FileService> {
    Router::new()
        .route("/file/internal/migrations/state", get(handler))
        .layer(Extension(state.pool_ref().clone()))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_has_two_migrations() {
        let s = FileMigrationState;
        assert_eq!(s.service_name(), "file-service");
        assert_eq!(s.dialect(), "mysql");
        assert_eq!(s.migrations().len(), 2);
        assert_eq!(s.migrations()[0].version, "V001");
        assert_eq!(s.migrations()[1].version, "V002");
        assert_eq!(s.migrations()[0].marker_table, "file_files");
        assert_eq!(
            s.migrations()[1].marker_table,
            "file_migration_v002_applied"
        );
    }
}
