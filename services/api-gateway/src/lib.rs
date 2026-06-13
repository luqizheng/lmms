pub mod blacklist;
pub mod config;
pub mod discovery;
pub mod jwks;
pub mod jwt;
pub mod middleware;
pub mod rewrite;
pub mod router;
pub mod routes_loader;

pub use config::{GatewayConfig, RewriteSpec, RouteConfig, RouteFile, RouteTable};
pub use discovery::{ServiceDiscovery, ServiceInstance};
pub use jwt::{JwtClaims, JwtValidator};
pub use router::RouterConfig;

use axum::{routing::get, Router};
use redis::aio::MultiplexedConnection;
use std::sync::Arc;

pub struct AppState {
    pub jwt_validator: Arc<JwtValidator>,
    pub redis_conn: MultiplexedConnection,
    pub redis_prefix: String,
    pub router_cfg: Arc<RouterConfig>,
    pub route_table: Arc<arc_swap::ArcSwap<RouteTable>>,
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
                match_pattern: "/a".into(),
                service: "s".into(),
                public: false,
                rewrite: vec![],
            }],
        }));
        let cur = swap.load();
        assert_eq!(cur.routes.len(), 1);
        assert_eq!(cur.routes[0].service, "s");
    }
}
