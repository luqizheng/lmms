use std::sync::Arc;
use common_config::Loader;
use common_http::{NoopObserver, ResilientClient, ResilienceConfig};
use token_service::openapi::ApiDoc;
use token_service::routes;
use token_service::service::oauth_client_svc;
use token_service::{
    TokenIssuer, TokenServerConfig, TokenServerState,
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

    let (private_key_pem, public_key_pem) = load_or_generate_keys(&cfg.jwt_private_key_path, &cfg.jwt_public_key_path);

    let issuer = Arc::new(
        TokenIssuer::new(&private_key_pem, &cfg.issuer).expect("failed to create token issuer"),
    );

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

// We are the JWKS source. Self-loopback via refresh() would race with
    // axum::serve binding (the listener isn't accepting yet) and leave the
    // cache empty on every restart, breaking inbound service-jwt verify.
    let jwks_cache: std::sync::Arc<common_jwt::JWKSCache> =
        std::sync::Arc::new(common_jwt::JWKSCache::from_pem(&public_key_pem)
            .expect("JWKSCache::from_pem"));

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
        private_key_pem: private_key_pem.clone(),
        bootstrap_tokens: std::env::var("SERVICE_BOOTSTRAP_TOKENS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        jwks: jwks_cache,
    };
    if state.bootstrap_tokens.is_empty() {
        tracing::warn!("SERVICE_BOOTSTRAP_TOKENS not set; POST /tokens/service-jwt will reject all callers");
    }

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
        .with_state(state.clone());

    if swagger_enabled {
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
        );
        tracing::info!("swagger ui enabled at /swagger-ui");
    }

    // Phase 5-B: service-jwt endpoint (bootstrap-token auth).
    app = app.merge(token_service::service_jwt_routes(state.clone()));

    // /tokens/user-jwt is service-internal only — must require service-jwt
    // with aud=token-service, otherwise anyone on the network could mint
    // user JWTs signed by this service.
    app = app.merge(
        axum::Router::new()
            .route(
                "/tokens/user-jwt",
                axum::routing::post(routes::user_jwt::handle_user_jwt),
            )
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                authz::verify_aud_principal_middleware::<token_service::TokenServerState>("token-service"),
            ))
            .with_state(state.clone()),
    );

    app = app.merge(
        token_service::routes::migration_state::routes(state.clone())
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                authz::verify_aud_principal_middleware::<token_service::TokenServerState>("token-service"),
            )),
    );

    
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        tracing::info!("received shutdown signal, deregistering from consul");
        std::process::exit(0);
    });

    fn load_or_generate_keys(private_key_path: &str, public_key_path: &str) -> (String, String) {
        match (std::fs::read_to_string(private_key_path), std::fs::read_to_string(public_key_path)) {
            (Ok(private), Ok(public)) => {
                tracing::info!("loaded existing JWT keys from files");
                (private, public)
            }
            _ => {
                tracing::warn!("JWT keys not found, generating new key pair");
                let key_pair = jwt_simple::prelude::RS256KeyPair::generate(2048)
                    .expect("failed to generate RSA key pair");
                let private_pem = key_pair.to_pem().expect("failed to encode private key");
                let public_key = key_pair.public_key();
                let public_pem = public_key.to_pem().expect("failed to encode public key");

                if let Some(parent) = std::path::Path::new(private_key_path).parent() {
                    std::fs::create_dir_all(parent).expect("failed to create keys directory");
                }
                std::fs::write(private_key_path, &private_pem)
                    .expect("failed to write private key");
                std::fs::write(public_key_path, &public_pem)
                    .expect("failed to write public key");
                tracing::info!("generated new JWT keys and saved to files");
                (private_pem, public_pem)
            }
        }
    }

    tracing::info!("token-service listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
