pub mod auth;
pub mod internal_users;
pub mod org_units;
pub mod permissions;
pub mod roles;
pub mod tenants;
pub mod users;

pub use internal_users::TenantIdQuery;

use axum::{Router, middleware as axum_mw, routing::get};
use authz::verify_aud_principal_middleware;
use crate::state::AppState;
use prometheus::{Encoder, TextEncoder};

pub fn build_router(state: AppState) -> Router {
    async fn metrics() -> axum::response::Html<String> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        axum::response::Html(String::from_utf8(buffer).unwrap())
    }

    // Phase 5-B: gate /iam/internal/* on a valid service-jwt with
    // aud="iam-service". Other routes (auth, users, etc.) still use
    // Principal::from_token without aud check (gateway user-jwt).
    let internal_users = internal_users::routes(state.clone())
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            verify_aud_principal_middleware::<AppState>("iam-service"),
        ));

    Router::new()
        .merge(auth::routes(state.clone()))
        .merge(users::routes(state.clone()))
        .merge(roles::routes(state.clone()))
        .merge(permissions::routes(state.clone()))
        .merge(org_units::routes(state.clone()))
        .merge(tenants::routes(state.clone()))
        .merge(internal_users)
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route_layer(axum_mw::from_fn(common_telemetry_axum::trace_request_middleware))
}

async fn health() -> &'static str {
    "OK"
}
