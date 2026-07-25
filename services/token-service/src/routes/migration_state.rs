//! GET /token/internal/migrations/state — token-service schema 状态查询。
//!
//! Phase 10 G3 接入。

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router, routing::get};
use common_db::migration_state::{self, MigrationDescriptor, MigrationState, MigrationStateResponse};
use serde_json::json;
use sqlx::MySqlPool;
use utoipa::path as to_path;

use crate::TokenServerState;
use crate::db::DbPool;

pub struct TokenMigrationState;

impl MigrationState for TokenMigrationState {
    fn service_name(&self) -> &'static str { "token-service" }
    fn dialect(&self) -> &'static str { "mysql" }
    fn migrations(&self) -> &'static [MigrationDescriptor] {
        &[MigrationDescriptor { version: "V001", name: "init", marker_table: "token_oauth_clients" }]
    }
}

async fn probe_applied(pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    let state = TokenMigrationState;
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
    path = "/token/internal/migrations/state",
    summary = "Query token-service migration state",
    responses(
        (status = 200, body = MigrationStateResponse, description = "schema state"),
        (status = 401, description = "missing or invalid service-jwt"),
        (status = 503, description = "db_unreachable"),
    ),
    security(("bearer_auth" = [])),
    tag = "internal",
)]
pub async fn handler(State(state): State<TokenServerState>) -> impl IntoResponse {
    let DbPool::Mysql(pool) = state.db;
    let body: Result<MigrationStateResponse, (StatusCode, Json<serde_json::Value>)> =
        match probe_applied(&pool).await {
            Ok(applied) => Ok(migration_state::build_response(&TokenMigrationState, applied)),
            Err(e) => Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "service": "token-service",
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

pub fn routes(state: TokenServerState) -> Router {
    Router::new()
        .route("/token/internal/migrations/state", get(handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_has_one_migration() {
        let s = TokenMigrationState;
        assert_eq!(s.service_name(), "token-service");
        assert_eq!(s.dialect(), "mysql");
        assert_eq!(s.migrations().len(), 1);
        assert_eq!(s.migrations()[0].version, "V001");
        assert_eq!(s.migrations()[0].marker_table, "token_oauth_clients");
    }
}