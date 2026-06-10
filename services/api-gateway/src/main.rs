mod cli;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware as axum_mw;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use api_gateway::{
    AppState, GatewayConfig, JwtValidator, RouteConfig, RouteTable, RouterConfig,
    ServiceDiscovery, blacklist, middleware, rewrite, router, routes_loader,
};
use cli::{Cli, Cmd};

fn match_pattern(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        let api_prefix = format!("/api{}", prefix);
        path == api_prefix || path.starts_with(&format!("{}/", api_prefix))
    } else {
        path == format!("/api{}", pattern)
    }
}

fn find_route(routes: &[RouteConfig], path: &str) -> Option<RouteConfig> {
    routes
        .iter()
        .find(|r| match_pattern(path, &r.match_pattern))
        .cloned()
}

fn load_global_config() -> GatewayConfig {
    let b = ::config::Config::builder()
        .add_source(::config::File::with_name("config/api-gateway").required(false))
        .add_source(
            ::config::Environment::with_prefix("GW")
                .prefix_separator("_")
                .separator("__"),
        );
    b.build()
        .unwrap()
        .try_deserialize::<GatewayConfig>()
        .unwrap()
}

fn print_routes(table: &RouteTable) {
    println!("routes:");
    for r in &table.routes {
        println!(
            "  - {}  ->  {} ({}{})",
            r.match_pattern,
            r.service,
            if r.public { "public" } else { "protected" },
            if r.rewrite.is_empty() {
                String::new()
            } else {
                format!(", {} rewrites", r.rewrite.len())
            }
        );
    }
}

fn do_reload(
    dir: &Path,
    swap: &arc_swap::ArcSwap<RouteTable>,
) -> Result<usize, routes_loader::LoadError> {
    let t = routes_loader::load_routes(dir)?;
    let n = t.routes.len();
    swap.store(Arc::new(t));
    Ok(n)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "api_gateway=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Validate { routes } => match routes_loader::load_routes(&routes) {
            Ok(t) => {
                println!("OK: {} routes from {}", t.routes.len(), routes.display());
                print_routes(&t);
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("FAIL: {e}");
                std::process::exit(1);
            }
        },
        Cmd::Serve {
            routes,
            admin_port,
        } => serve(routes, admin_port).await,
    }
}

async fn serve(routes_dir: PathBuf, admin_port: u16) {
    let cfg = load_global_config();
    let table = routes_loader::load_routes(&routes_dir)
        .unwrap_or_else(|e| panic!("failed to load routes from {}: {}", routes_dir.display(), e));

    println!("========== api-gateway config ==========");
    println!("server.host          : {}", cfg.server.host);
    println!("server.port          : {}", cfg.server.port);
    println!("issuer               : {}", cfg.issuer);
    println!("jwks_public_key_path : {}", cfg.jwks_public_key_path);
    println!("redis.url            : {}", cfg.redis.url);
    println!("redis.prefix         : {}", cfg.redis.prefix);
    println!("consul.address       : {}", cfg.consul_addr);
    println!("routes.dir           : {}", routes_dir.display());
    println!("admin.port           : {}", admin_port);
    print_routes(&table);
    println!("========================================");

    let public_key_pem =
        std::fs::read_to_string(&cfg.jwks_public_key_path).expect("failed to read public key");

    let jwt_validator =
        JwtValidator::new(&public_key_pem, &cfg.issuer).expect("failed to create JWT validator");

    let redis_conn = blacklist::connect(&cfg.redis.url)
        .await
        .expect("failed to connect to redis");
    tracing::info!("connected to redis at {}", cfg.redis.url);

    let discovery = Arc::new(ServiceDiscovery::new(&cfg.consul_addr));

    let router_cfg = Arc::new(RouterConfig {
        discovery: discovery.clone(),
        http_client: reqwest::Client::new(),
    });

    let route_table = Arc::new(arc_swap::ArcSwap::from_pointee(table));

    // SIGHUP handler (POSIX only)
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::hangup()) {
            Ok(mut sighup) => {
                let rt = route_table.clone();
                let dir = routes_dir.clone();
                tokio::spawn(async move {
                    while sighup.recv().await.is_some() {
                        match do_reload(&dir, &rt) {
                            Ok(n) => tracing::info!("SIGHUP reload ok: {} routes", n),
                            Err(e) => tracing::error!("SIGHUP reload failed: {e}"),
                        }
                    }
                });
                tracing::info!("SIGHUP handler installed");
            }
            Err(e) => tracing::warn!("failed to install SIGHUP handler: {e}"),
        }
    }

    let state = Arc::new(AppState {
        jwt_validator: Arc::new(jwt_validator),
        redis_conn,
        redis_prefix: cfg.redis.prefix.clone(),
        router_cfg: router_cfg.clone(),
        route_table: route_table.clone(),
    });

    let health_route = Router::new().route(
        "/health",
        get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
    );

    let api_routes = Router::new()
        .route(
            "/*key",
            get(proxy_handler)
                .post(proxy_handler)
                .put(proxy_handler)
                .patch(proxy_handler)
                .delete(proxy_handler),
        )
        .route_layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    let app = Router::new()
        .merge(health_route)
        .nest("/api", api_routes)
        .with_state(state);

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    tracing::info!("api-gateway listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    if admin_port > 0 {
        spawn_admin_server(admin_port, routes_dir.clone(), route_table.clone()).await;
    }

    axum::serve(listener, app).await.unwrap();
}

async fn spawn_admin_server(
    port: u16,
    routes_dir: PathBuf,
    route_table: Arc<arc_swap::ArcSwap<RouteTable>>,
) {
    let admin = Router::new().route(
        "/__admin/reload",
        post(move || {
            let rt = route_table.clone();
            let dir = routes_dir.clone();
            async move {
                match do_reload(&dir, &rt) {
                    Ok(n) => (
                        StatusCode::OK,
                        axum::Json(serde_json::json!({"reloaded": n})),
                    )
                        .into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(serde_json::json!({"error": e.to_string()})),
                    )
                        .into_response(),
                }
            }
        }),
    );

    let addr = format!("0.0.0.0:{}", port);
    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            tracing::info!("admin server listening on {}", addr);
            tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, admin).await {
                    tracing::error!("admin server error: {e}");
                }
            });
        }
        Err(e) => tracing::warn!("failed to bind admin server on {}: {}", addr, e),
    }
}

async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    mut request: Request,
) -> Response {
    let path = request.uri().path().to_string();
    let table = state.route_table.load();

    let route = match find_route(&table.routes, &path) {
        Some(r) => r,
        None => {
            tracing::warn!("no route matched for path: {}", path);
            return (StatusCode::NOT_FOUND, "unknown service").into_response();
        }
    };

    if let Err(resp) = rewrite::apply(&route.rewrite, &mut request) {
        return resp;
    }

    router::forward_to_service(state.router_cfg.clone(), request, &route.service).await
}
