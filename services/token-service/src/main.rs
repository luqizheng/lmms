use std::sync::Arc;
use token_service::routes;
use token_service::service::oauth_client_svc;
use token_service::{
    load_config, ConsulRegistrar, TokenIssuer, TokenServerConfig, TokenServerState,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use prometheus::{Encoder, TextEncoder};
use utoipa::{OpenApi, ToSchema};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "token_service=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg: TokenServerConfig = load_config("config/config.yaml")
        .or_else(|_| load_config("config/token-service.yaml"))
        .expect("failed to load config.yaml");

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
        http_client: {
            use hyper_util::client::legacy::connect::HttpConnector;
            use hyper_util::client::legacy::Client;
            use hyper_util::rt::TokioExecutor;
            Client::builder(TokioExecutor::new()).build(HttpConnector::new())
        },
        user_service_url: cfg.user_service_url.clone(),
    };

    async fn metrics() -> axum::response::Html<String> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        axum::response::Html(String::from_utf8(buffer).unwrap())
    }

    #[derive(OpenApi)]
    #[openapi()]
    struct ApiDoc;

    let app = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .route("/metrics", axum::routing::get(metrics))
        .route(
            "/api/tokens/access",
            axum::routing::post(routes::access::handle_access),
        )
        .route(
            "/api/tokens/refresh",
            axum::routing::post(routes::refresh::handle_refresh),
        )
        .route(
            "/api/tokens/revoke",
            axum::routing::post(routes::revoke::handle_revoke),
        )
        .route(
            "/api/tokens/resource",
            axum::routing::post(routes::resource::handle_resource),
        )
        .route(
            "/api/tokens/.well-known/jwks.json",
            axum::routing::get(routes::jwks::handle_jwks),
        )
        .route(
            "/.well-known/jwks.json",
            axum::routing::get(routes::jwks::handle_jwks),
        )
        .route(
            "/api/api-keys",
            axum::routing::post(routes::api_key::handle_create_api_key),
        )
        .route(
            "/api/v1/tokens/user-jwt",
            axum::routing::post(routes::user_jwt::handle_user_jwt),
        )
        .route(
            "/api/v1/admin/oauth-clients",
            axum::routing::get(routes::admin::handle_list)
                .post(routes::admin::handle_create),
        )
        .route(
            "/api/v1/admin/oauth-clients/{client_id}",
            axum::routing::put(routes::admin::handle_update)
                .delete(routes::admin::handle_delete),
        )
        .route(
            "/api/v1/admin/oauth-clients/{client_id}/reset-secret",
            axum::routing::post(routes::admin::handle_reset_secret),
        )
        .with_state(state);

    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        &cfg.consul.service_id_file,
    );
    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    match consul_registrar
        .register(&cfg.server.host, cfg.server.port)
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
