pub mod blacklist;
pub mod config;
pub mod middleware;
pub mod perm_epoch_cache;
pub mod openapi;
pub mod perm_consumer;
pub mod perm_epoch;
pub mod resilience_metrics;
pub mod router;
pub mod routes_loader;
pub mod template;

pub use config::{GatewayConfig, RouteConfig, RouteFile, RouteTable};
pub use middleware::JwtClaims;
pub use router::RouterConfig;

use axum::{routing::get, Router};
use redis::aio::MultiplexedConnection;
use std::sync::Arc;

pub struct AppState {
    pub jwks_cache: Arc<digua_common_jwt::JWKSCache>,
    pub perm_epoch_cache: perm_epoch_cache::PermEpochCache,
    pub redis_conn: MultiplexedConnection,
    pub redis_prefix: String,
    pub router_cfg: Arc<RouterConfig>,
    pub route_table: Arc<arc_swap::ArcSwap<RouteTable>>,
    pub route_limiters: crate::middleware::rate_limit::RouteLimiterSet,
}

pub async fn create_app(state: AppState) -> Router {
    let health_route = Router::new().route(
        "/health",
        get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
    );
    Router::new()
        .merge(health_route)
        .with_state(Arc::new(state))
}

pub async fn create_test_app() -> Router {
    let health_route = Router::new().route(
        "/health",
        get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
    );
    Router::new().merge(health_route).with_state(Arc::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_table_swap() {
        let swap = arc_swap::ArcSwap::from_pointee(RouteTable::default());
        swap.store(Arc::new(RouteTable {
            routes: vec![RouteConfig {
                upstream: "/v1/a".into(),
                downstream: "/a".into(),
                service: "s".into(),
                public: false,
                ws: false,
                headers: Default::default(),
                timeout_ms: None,
            }],
        }));
        let cur = swap.load();
        assert_eq!(cur.routes.len(), 1);
        assert_eq!(cur.routes[0].service, "s");
    }
}
