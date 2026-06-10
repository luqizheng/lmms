use axum::routing::{get, post};
use axum::Router;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use token_service::routes;
use token_service::store;
use token_service::ClientStore;
use token_service::ConsulRegistrar;
use token_service::TokenIssuer;
use token_service::TokenServerConfig;
use token_service::TokenServerState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "token_server=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = {
        let b = ::config::Config::builder();
        let b = b.add_source(::config::File::with_name("config/token-server").required(false));
        let b = b.add_source(
            ::config::Environment::with_prefix("TS")
                .prefix_separator("_")
                .separator("__"),
        );
        b.build()
            .unwrap()
            .try_deserialize::<TokenServerConfig>()
            .unwrap()
    };

    let clients_path = if cfg.clients_file_path.is_empty() {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        exe_dir
            .join("config")
            .join("clients.json")
            .to_string_lossy()
            .to_string()
    } else {
        cfg.clients_file_path.clone()
    };
    if let Some(parent) = std::path::Path::new(&clients_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    println!("========== token-server config ==========");
    println!("server.host           : {}", cfg.server.host);
    println!("server.port           : {}", cfg.server.port);
    println!("issuer                : {}", cfg.issuer);
    println!("clients_file_path     : {}", clients_path);
    println!("jwt_private_key_path  : {}", cfg.jwt_private_key_path);
    println!("jwt_public_key_path   : {}", cfg.jwt_public_key_path);
    println!(
        "default_resource_ttl  : {} sec",
        cfg.default_resource_ttl_sec
    );
    println!("user_service_url      : {}", cfg.user_service_url);
    println!("redis.url             : {}", cfg.redis.url);
    println!("redis.prefix          : {}", cfg.redis.prefix);
    println!("consul.address        : {}", cfg.consul.addr);
    println!("consul.service_name   : {}", cfg.consul.service_name);
    println!("consul.service_id_file: {}", cfg.consul.service_id_file);
    println!("=========================================");
    tracing::info!(
        "config loaded, listening on {}:{}",
        cfg.server.host,
        cfg.server.port
    );
    let client_store =
        Arc::new(ClientStore::load(&clients_path).expect("failed to load clients.json"));

    let private_key_pem =
        std::fs::read_to_string(&cfg.jwt_private_key_path).expect("failed to read private key");
    let public_key_pem =
        std::fs::read_to_string(&cfg.jwt_public_key_path).expect("failed to read public key");

    let issuer = Arc::new(
        TokenIssuer::new(&private_key_pem, &cfg.issuer).expect("failed to create token issuer"),
    );

    let redis_conn = store::redis::connect(&cfg.redis.url)
        .await
        .expect("failed to connect to redis");
    tracing::info!("connected to redis at {}", cfg.redis.url);

    let state = TokenServerState {
        client_store,
        issuer,
        issuer_str: cfg.issuer.clone(),
        redis_conn,
        redis_prefix: cfg.redis.prefix.clone(),
        jwt_public_key: public_key_pem,
        default_resource_ttl_sec: cfg.default_resource_ttl_sec,
        http_client: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
        user_service_url: cfg.user_service_url.clone(),
    };

    let app = Router::new()
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .route("/api/tokens/access", post(routes::access::handle_access))
        .route("/api/tokens/refresh", post(routes::refresh::handle_refresh))
        .route("/api/tokens/revoke", post(routes::revoke::handle_revoke))
        .route(
            "/api/tokens/resource",
            post(routes::resource::handle_resource),
        )
        .route(
            "/api/tokens/.well-known/jwks.json",
            get(routes::jwks::handle_jwks),
        )
        .route(
            "/api/api-keys",
            post(routes::api_key::handle_create_api_key),
        )
        .with_state(state);

    let consul_registrar = ConsulRegistrar::new(&cfg.consul);

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    match consul_registrar
        .register(&cfg.server.host, cfg.server.port)
        .await
    {
        Ok(_) => tracing::info!("token-server registered with consul"),
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

    tracing::info!("token-server listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
