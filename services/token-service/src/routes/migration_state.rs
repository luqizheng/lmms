//! GET /token/internal/migrations/state — token-service schema 状态查询。
//!
//! Phase 10 G3 接入。

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use digua_common_db::migration_state::{
    self, MigrationDescriptor, MigrationState, MigrationStateResponse,
};
use serde_json::json;
use utoipa::path as to_path;

use crate::db::DbPool;
use crate::TokenServerState;

pub struct TokenMigrationState;

impl MigrationState for TokenMigrationState {
    fn service_name(&self) -> &'static str {
        "token-service"
    }
    fn dialect(&self) -> &'static str {
        "mysql"
    }
    fn migrations(&self) -> &'static [MigrationDescriptor] {
        &[
            MigrationDescriptor {
                version: "V001",
                name: "init",
                marker_table: "token_oauth_clients",
            },
            // V002 是 ALTER 型迁移：marker 用它自己的哨兵表
            // （token_migration_v002_applied）。不能沿用 token_oauth_clients——
            // 那张表 V001 已在每个存量库里建好，用它探测会把没跑过 V002 的库
            // 误报成"V002 已应用"。
            MigrationDescriptor {
                version: "V002",
                name: "add_client_user_and_expiry",
                marker_table: "token_migration_v002_applied",
            },
            // V003 建 token_bootstrap_tokens 表（bootstrap token 持久化）。
            MigrationDescriptor {
                version: "V003",
                name: "bootstrap_tokens",
                marker_table: "token_migration_v003_applied",
            },
            // V004 建 token_service_jwt_grants 表（service-jwt owner→audience
            // 授权，WP-LMMS-SERVICE-JWT-AUDIENCE-02）。
            MigrationDescriptor {
                version: "V004",
                name: "service_jwt_grants",
                marker_table: "token_migration_v004_applied",
            },
        ]
    }
}

async fn probe_applied(pool: &DbPool) -> Result<Vec<String>, sqlx::Error> {
    let state = TokenMigrationState;
    let mut applied = Vec::with_capacity(state.migrations().len());
    for m in state.migrations() {
        // 统一入口：内部按后端选 probe_marker_{mysql,postgres}，不再手写方言 match。
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
    let body: Result<MigrationStateResponse, (StatusCode, Json<serde_json::Value>)> =
        match probe_applied(&state.db).await {
            Ok(applied) => Ok(migration_state::build_response(
                &TokenMigrationState,
                applied,
            )),
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
    fn token_has_four_migrations() {
        let s = TokenMigrationState;
        assert_eq!(s.service_name(), "token-service");
        assert_eq!(s.dialect(), "mysql");
        assert_eq!(s.migrations().len(), 4);
        assert_eq!(s.migrations()[0].version, "V001");
        assert_eq!(s.migrations()[0].marker_table, "token_oauth_clients");
        assert_eq!(s.migrations()[1].version, "V002");
        assert_eq!(
            s.migrations()[1].marker_table,
            "token_migration_v002_applied"
        );
        assert_eq!(s.migrations()[2].version, "V003");
        assert_eq!(
            s.migrations()[2].marker_table,
            "token_migration_v003_applied"
        );
        assert_eq!(s.migrations()[3].version, "V004");
        assert_eq!(
            s.migrations()[3].marker_table,
            "token_migration_v004_applied"
        );
    }

    /// 描述符必须与 `db::MIGRATIONS` 逐条对齐（版本 + marker），
    /// 否则 `/internal/migrations/state` 会把未跑的迁移误报成已应用。
    #[test]
    fn migration_descriptors_match_db_migrations() {
        let s = TokenMigrationState;
        let descriptors = s.migrations();
        assert_eq!(descriptors.len(), crate::db::MIGRATIONS.len());
        for (d, m) in descriptors.iter().zip(crate::db::MIGRATIONS.iter()) {
            assert_eq!(d.version, m.version);
            assert_eq!(d.marker_table, m.marker_table);
        }
    }
}
