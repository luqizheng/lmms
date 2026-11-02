//! GET /prue/internal/migrations/state — prue_backend schema 状态查询。
//!
//! Phase 10 G3 接入。prue_backend 默认是 MySQL + SQLite 双 dialect 服务，
//! 另可通过 `--features postgres` 编入第三种方言。
//! 1 migration × 3 dialects：V001 init (marker: prue_menu_configs)。
//!
//! 注：`dialect()` 仍返回 `"mysql+sqlite"`（默认编译的方言集），因为
//! `scripts/check-migration-state.js` / `scripts/e2e-migration-state.js` / `docs-site`
//! 都按该字面量断言；引入 postgres 是本轮新增的可选方言，不改变默认集，故不动该字符串。

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
        // 统一入口：内部按后端选 probe_marker_{mysql,postgres,sqlite}，不再手写 match。
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
