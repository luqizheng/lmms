pub mod auth;
pub mod menus;
pub mod org_units;
pub mod permissions;
pub mod roles;
pub mod sessions;
pub mod users;

use axum::{Router, routing::get};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(auth::routes(state.clone()))
        .merge(users::routes(state.clone()))
        .merge(roles::routes(state.clone()))
        .merge(permissions::routes(state.clone()))
        .merge(menus::routes(state.clone()))
        .merge(org_units::routes(state.clone()))
        .merge(sessions::routes(state))
        .route("/health", get(health))
}

async fn health() -> &'static str {
    "OK"
}
