use std::sync::Arc;
use common_config::Loader;
use common_http::{NoopObserver, ResilientClient, ResilienceConfig};
use token_service::openapi::ApiDoc;
use token_service::routes;
use token_service::service::oauth_client_svc;
use consul_registrar::Upstream;
use token_service::{
    ConsulRegistrar, TokenIssuer, TokenServerConfig, TokenServerState,
};
use prometheus::{Encoder, TextEncoder};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    common_telemetry::init_tracing("token-service");

    let mut loader = Loader::new("TOKEN").with_file("config/config.yaml");
    if let (Ok(url), Ok(ns)) = (
        std::env::var("CONFIG_SERVICE_URL"),
        std::env::var("CONFIG_SERVICE_NAMESPACE"),
    ) {
        loader = loader.with_config_service(url, ns);
    }
    let cfg: TokenServerConfig = loader
        .load()
        .expect("failed to load config (tried config/config.yaml)");

    println!("========== token-service config ==========");
    println!("server.host           : {}", cfg.server.host);
    println!("server.port           : {}", cfg.server.port);
    println!("issuer                : {}", cfg.issuer);
    println!("database.db_type      : {:?}", cfg.database.db_type);
    println!("database.url          : {}", cfg.database.url);
    println!("jwt_private_key_path  : {}", cfg.jwt_private_key_path);
    println!("jwt_public_key_path   : {}", cfg.jwt_public_key_path);
    println!("user_service_url      : {}", cfg.user_service_url);
    println!("consul.address        : {}", cfg.consul.addr);
    println!("=========================================");

    let db = token_service::db::init_pool(&cfg.database)
        .await
        .expect("failed to connect to mysql");
    tracing::info!("connected to mysql at {:?}", cfg.database.url);

    token_service::db::ensure_schema(&db)
        .await
        .expect("ensure schema");

    let seeded = oauth_client_svc::seed_default_clients(&db)
        .await
        .expect("seed default clients");
    if seeded > 0 {
        tracing::info!("seeded {} default oauth clients", seeded);
    }

    let private_key_pem =
        std::fs::read_to_string(&cfg.jwt_private_key_path).expect("failed to read private key");
    let public_key_pem =
        std::fs::read_to_string(&cfg.jwt_public_key_path).expect("failed to read public key");

    let issuer = Arc::new(
        TokenIssuer::new(&private_key_pem, &cfg.issuer).expect("failed to create token issuer"),
    );

    let state = TokenServerState {
        db: db.clone(),
        issuer,
        issuer_str: cfg.issuer.clone(),
        jwt_public_key: public_key_pem,
        default_resource_ttl_sec: cfg.default_resource_ttl_sec,
        http_client: Arc::new(ResilientClient::new(
            reqwest::Client::new(),
            ResilienceConfig::default(),
            Arc::new(NoopObserver),
        )),
        user_service_url: cfg.user_service_url.clone(),
    };

    async fn metrics() -> axum::response::Html<String> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        axum::response::Html(String::from_utf8(buffer).unwrap())
    }

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    let mut app = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .route("/metrics", axum::routing::get(metrics))
        .route(
            "/tokens/access",
            axum::routing::post(routes::access::handle_access),
        )
        .route(
            "/tokens/refresh",
            axum::routing::post(routes::refresh::handle_refresh),
        )
        .route(
            "/tokens/revoke",
            axum::routing::post(routes::revoke::handle_revoke),
        )
        .route(
            "/tokens/resource",
            axum::routing::post(routes::resource::handle_resource),
        )
        .route(
            "/tokens/.well-known/jwks.json",
            axum::routing::get(routes::jwks::handle_jwks),
        )
        .route(
            "/.well-known/jwks.json",
            axum::routing::get(routes::jwks::handle_jwks),
        )
        .route(
            "/api-keys",
            axum::routing::post(routes::api_key::handle_create_api_key)
                .get(routes::api_key::handle_list_api_keys),
        )
        .route(
            "/tokens/user-jwt",
            axum::routing::post(routes::user_jwt::handle_user_jwt),
        )
        .route(
            "/admin/oauth-clients",
            axum::routing::get(routes::admin::handle_list)
                .post(routes::admin::handle_create),
        )
        .route(
            "/admin/oauth-clients/{client_id}",
            axum::routing::put(routes::admin::handle_update)
                .delete(routes::admin::handle_delete),
        )
        .route(
            "/admin/oauth-clients/{client_id}/reset-secret",
            axum::routing::post(routes::admin::handle_reset_secret),
        )
        .route_layer(axum::middleware::from_fn(
            common_telemetry_axum::trace_request_middleware,
        ))
        .with_state(state);

    if swagger_enabled {
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
        );
        tracing::info!("swagger ui enabled at /swagger-ui");
    }

    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        &cfg.consul.service_id_file,
    )
    .with_upstreams(vec![Upstream::new("iam-service", 9081)]);
    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    match consul_registrar
        .register_discovered(cfg.server.port, &cfg.consul.service_name)
        .await
    {
        Ok(_) => tracing::info!("token-service registered with consul"),
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

    tracing::info!("token-service listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
