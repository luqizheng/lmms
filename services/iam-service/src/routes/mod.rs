pub mod auth;
pub mod menus;
pub mod org_units;
pub mod permissions;
pub mod roles;
pub mod sessions;
pub mod users;

use axum::{Router, routing::get};
use crate::state::AppState;
use prometheus::{Encoder, TextEncoder};
use utoipa::{OpenApi};
use utoipa_swagger_ui::SwaggerUi;

pub fn build_router(state: AppState) -> Router {
    async fn metrics() -> axum::response::Html<String> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }

    #[derive(OpenApi)]
    #[openapi(
        paths(
            auth::login,
            users::get_users,
            users::get_user,
            users::create_user,
            users::update_user,
            users::delete_user,
            roles::get_roles,
            roles::get_role,
            roles::create_role,
            roles::update_role,
            roles::delete_role,
            permissions::get_permissions,
            permissions::get_permission,
            permissions::create_permission,
            permissions::update_permission,
            permissions::delete_permission,
            menus::get_menus,
            org_units::get_org_units,
            sessions::get_sessions,
            sessions::revoke_session,
        ),
        components(
        )
    )]
    struct ApiDoc;

    Router::new()
        .merge(auth::routes(state.clone()))
        .merge(users::routes(state.clone()))
        .merge(roles::routes(state.clone()))
        .merge(permissions::routes(state.clone()))
        .merge(menus::routes(state.clone()))
        .merge(org_units::routes(state.clone()))
        .merge(sessions::routes(state))
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
}

async fn health() -> &'static str {
    "OK"
}
