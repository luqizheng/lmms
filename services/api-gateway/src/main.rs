mod cli;

use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware as axum_mw;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use prometheus::{Encoder, TextEncoder};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use utoipa::OpenApi;

use api_gateway::template::Template;
use api_gateway::{
    blacklist, middleware, router, routes_loader, AppState, GatewayConfig, JwtValidator,
    RouteConfig, RouteTable, RouterConfig,
};
use api_gateway::resilience_metrics::GatewayMetricsObserver;
use cli::{Cli, Cmd};
use common_http::ResilientClient;
use consul_registrar::ConsulRegistrar;

fn find_and_render(
    routes: &[RouteConfig],
    path_and_query: &str,
) -> Option<(RouteConfig, String)> {
    for r in routes {
        let up = match Template::parse(&r.upstream) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let dict = match up.extract(path_and_query) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let down = match Template::parse(&r.downstream) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let target = match down.render(&dict) {
            Ok(s) => s,
            Err(_) => continue,
        };
        return Some((r.clone(), target));
    }
    None
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
            "  - {}  ->  {} ({})  =>  {}",
            r.upstream,
            r.service,
            if r.public { "public" } else { "protected" },
            r.downstream,
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
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    common_telemetry::init_tracing("api-gateway");
    middleware::rate_limit::init_metrics();

    let cli = Cli::parse();
    let result = match cli.cmd {
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
        Cmd::Serve { routes, admin_port } => serve(routes, admin_port).await,
    };

    common_telemetry::shutdown();
    result
}

async fn serve(routes_dir: PathBuf, admin_port: u16) {
    let cfg = load_global_config();

    println!("========== api-gateway config ==========");
    println!("server.host          : {}", cfg.server.host);
    println!("server.port          : {}", cfg.server.port);
    println!("issuer               : {}", cfg.issuer);
    println!("jwks_url             : {}", cfg.jwks_url);
    println!("redis.url            : {}", cfg.redis.url);
    println!("redis.prefix         : {}", cfg.redis.prefix);
    println!("consul.address       : {}", cfg.consul_addr);
    println!("routes.dir           : {}", routes_dir.display());
    println!("admin.port           : {}", admin_port);

    if !routes_dir.exists() {
        tracing::warn!(
            "routes dir {} does not exist; creating it",
            routes_dir.display()
        );
        if let Err(e) = std::fs::create_dir_all(&routes_dir) {
            tracing::error!(
                "failed to create routes dir {}: {e}; starting with empty route table",
                routes_dir.display()
            );
        }
    }
    println!("routes.dir.exists()  : {}", routes_dir.exists());

    let table = routes_loader::load_routes(&routes_dir).unwrap_or_else(|e| {
        tracing::warn!(
            "failed to load routes from {}: {}; starting with empty route table",
            routes_dir.display(),
            e
        );
        RouteTable { routes: vec![] }
    });

    print_routes(&table);
    println!("========================================");

    let http_client = reqwest::Client::new();
    let public_key = api_gateway::jwks::fetch_rs256_key(&http_client, &cfg.jwks_url)
        .await
        .expect("failed to fetch JWKS public key");

    let jwt_validator =
        JwtValidator::from_key(public_key, &cfg.issuer).expect("failed to create JWT validator");

    let redis_conn = blacklist::connect(&cfg.redis.url)
        .await
        .expect("failed to connect to redis");
    tracing::info!("connected to redis at {}", cfg.redis.url);

    let resilient = Arc::new(ResilientClient::new(
        reqwest::Client::new(),
        cfg.resilience.clone(),
        Arc::new(GatewayMetricsObserver),
    ));
    let router_cfg = Arc::new(RouterConfig {
        resilient,
        egress_map: Arc::new(cfg.egress_map.clone()),
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
        route_limiters: middleware::rate_limit::RouteLimiterSet::from_config(
            &middleware::rate_limit::RateLimitConfig::default(),
        ),
    });

    // Phase 4.4: backfill consumer — subscribe iam.permission.changed and
    // set-if-greater perm_epoch, closing iam's direct-write fail-open window.
    {
        let conn = state.redis_conn.clone();
        let prefix = cfg.redis.prefix.clone();
        let ttl = cfg.perm_epoch_ttl_sec;
        let url = cfg.event_bus_url.clone();
        tokio::spawn(async move {
            if let Err(e) = api_gateway::perm_consumer::run(conn, prefix, ttl, url).await {
                tracing::warn!("perm_epoch consumer exited: {e}");
            }
        });
    }

    async fn metrics() -> axum::response::Html<String> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        axum::response::Html(String::from_utf8(buffer).unwrap())
    }

    let health_route = Router::new()
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .route("/metrics", get(metrics))
        .route(
            "/api-docs/openapi.json",
            get(|| async { axum::Json(api_gateway::openapi::ApiDoc::openapi()) }),
        );

    let api_routes = Router::new()
        .route(
            "/{*key}",
            get(proxy_handler)
                .post(proxy_handler)
                .put(proxy_handler)
                .patch(proxy_handler)
                .delete(proxy_handler),
        )
        .route_layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ))
        .route_layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::rate_limit::rate_limit_middleware,
        ))
        .route_layer(axum_mw::from_fn(
            common_telemetry_axum::trace_request_middleware,
        ))
        .without_v07_checks();

    let app = Router::new()
        .merge(health_route)
        .merge(api_routes)
        .with_state(state);

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    tracing::info!("api-gateway listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul_addr,
        "api-gateway",
        "/tmp/api-gateway-id",
    );
    match consul_registrar
        .register_discovered(cfg.server.port, "api-gateway")
        .await
    {
        Ok(_) => tracing::info!("api-gateway registered with consul"),
        Err(e) => tracing::warn!("failed to register with consul: {}", e),
    }
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        tracing::info!("received shutdown signal, deregistering from consul");
        let _ = consul_registrar.deregister().await;
        std::process::exit(0);
    });

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

async fn proxy_handler(State(state): State<Arc<AppState>>, mut request: Request) -> Response {
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(|q| q.to_string());
    let table = state.route_table.load();

    let (route, mut target) = match find_and_render(&table.routes, &path) {
        Some(x) => x,
        None => {
            let pq = request
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str().to_string())
                .unwrap_or_else(|| path.clone());
            tracing::warn!("no route matched for path: {}", pq);
            return (StatusCode::NOT_FOUND, "unknown service").into_response();
        }
    };

    if let Some(q) = query {
        if !target.contains('?') {
            target.push('?');
            target.push_str(&q);
        } else {
            target.push('&');
            target.push_str(&q);
        }
    }

    // 注入 headers（启动期已校验合法，运行期同样 graceful）
    for (k, v) in &route.headers {
        if let (Ok(name), Ok(val)) = (k.parse::<HeaderName>(), v.parse::<HeaderValue>()) {
            request.headers_mut().insert(name, val);
        }
    }

    // Inject W3C trace context so downstream services chain into our trace.
    opentelemetry::global::get_text_map_propagator(|propagator| {
        use opentelemetry::propagation::Injector;
        struct Hd<'a>(&'a mut axum::http::HeaderMap);
        impl<'a> Injector for Hd<'a> {
            fn set(&mut self, key: &str, value: String) {
                if let (Ok(name), Ok(val)) = (
                    key.parse::<axum::http::HeaderName>(),
                    axum::http::HeaderValue::from_str(&value),
                ) {
                    self.0.insert(name, val);
                }
            }
        }
        propagator.inject_context(&tracing::Span::current().context(), &mut Hd(request.headers_mut()));
    });

    router::forward_to_service(state.router_cfg.clone(), request, &route.service, &target).await
}
