use digua_common_http::{NoopObserver, ResilienceConfig, ResilientClient};
use prometheus::{Encoder, TextEncoder};
use std::sync::Arc;
use token_service::config::{load_config_with_env, DEFAULT_CONFIG_PATH};
use token_service::openapi::ApiDoc;
use token_service::routes;
use token_service::service::oauth_client_svc;
use token_service::{TokenIssuer, TokenServerState};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    digua_common_telemetry::init_tracing("token-service");

    let config_path =
        std::env::var("CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());

    // Optional: pull a HashMap from config-service and treat it as a
    // lower-priority env fallback. Process env still wins the final say.
    // Historical `digua_common_config::Loader` did the same thing via a
    // `set_override` layer; we replicate it here so the config-service
    // dogfood story still works after the Loader retirement.
    let remote_env: std::collections::HashMap<String, String> = fetch_remote_config_env();
    if !remote_env.is_empty() {
        tracing::info!(
            keys = remote_env.len(),
            "loaded remote config from CONFIG_SERVICE_URL (falls back under process env)"
        );
    }

    let env_lookup = move |k: &str| {
        // Process env wins; remote_env is a lower-priority fallback layer.
        std::env::var(k).ok().or_else(|| remote_env.get(k).cloned())
    };

    let cfg = match load_config_with_env(&config_path, env_lookup) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config (path={config_path}): {e}");
            std::process::exit(1);
        }
    };

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

    // 只建一次池、跑一次迁移（历史上这里被复制成两段，重复连库 + 重复迁移）。
    let db = token_service::db::init_pool(&cfg.database)
        .await
        .expect("failed to connect to database");
    tracing::info!(
        dialect = ?db.dialect(),
        "connected to database at {:?}",
        cfg.database.url
    );

    token_service::db::ensure_schema(&db)
        .await
        .expect("ensure schema");

    let seeded = oauth_client_svc::seed_default_clients(&db)
        .await
        .expect("seed default clients");
    if seeded > 0 {
        tracing::info!("seeded {} default oauth clients", seeded);
    }

    let (private_key_pem, public_key_pem) =
        load_or_generate_keys(&cfg.jwt_private_key_path, &cfg.jwt_public_key_path);

    let issuer = Arc::new(
        TokenIssuer::new(&private_key_pem, &cfg.issuer).expect("failed to create token issuer"),
    );

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    // We are the JWKS source. Self-loopback via refresh() would race with
    // axum::serve binding (the listener isn't accepting yet) and leave the
    // cache empty on every restart, breaking inbound service-jwt verify.
    let jwks_cache: std::sync::Arc<digua_common_jwt::JWKSCache> = std::sync::Arc::new(
        digua_common_jwt::JWKSCache::from_pem(&public_key_pem).expect("JWKSCache::from_pem"),
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
        tracing::warn!(
            "SERVICE_BOOTSTRAP_TOKENS not set; POST /tokens/service-jwt will reject all callers"
        );
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
        .route_layer(axum::middleware::from_fn(
            digua_common_telemetry_axum::trace_request_middleware,
        ))
        .with_state(state.clone());

    if swagger_enabled {
        app = app
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
        tracing::info!("swagger ui enabled at /swagger-ui");
    }

    // /api-keys 与 /admin/oauth-clients* 由各自的 routes() 构造器挂载——
    // 内部带 service-jwt 鉴权中间件（aud=token-service）。
    app = app.merge(routes::api_key::routes(state.clone()));
    app = app.merge(routes::admin::routes(state.clone()));

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
                digua_authz::verify_aud_principal_middleware::<token_service::TokenServerState>(
                    "token-service",
                ),
            ))
            .with_state(state.clone()),
    );

    app = app.merge(
        token_service::routes::migration_state::routes(state.clone()).layer(
            axum::middleware::from_fn_with_state(
                state.clone(),
                digua_authz::verify_aud_principal_middleware::<token_service::TokenServerState>(
                    "token-service",
                ),
            ),
        ),
    );

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        tracing::info!("received shutdown signal, deregistering from consul");
        std::process::exit(0);
    });

    fn load_or_generate_keys(private_key_path: &str, public_key_path: &str) -> (String, String) {
        match (
            std::fs::read_to_string(private_key_path),
            std::fs::read_to_string(public_key_path),
        ) {
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
                std::fs::write(public_key_path, &public_pem).expect("failed to write public key");
                tracing::info!("generated new JWT keys and saved to files");
                (private_pem, public_pem)
            }
        }
    }

    tracing::info!("token-service listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

/// Pull `HashMap<String, String>` from the optional remote config-service.
/// Failure is soft: any error is logged and treated as "no remote config",
/// so a config-service outage never blocks token-service startup.
///
/// This is the retirement path for `digua_common_config::Loader::with_config_service`;
/// the returned map is layered UNDER `std::env::var` in `main`, keeping the
/// process env's precedence unchanged.
fn fetch_remote_config_env() -> std::collections::HashMap<String, String> {
    let (Ok(url), Ok(ns)) = (
        std::env::var("CONFIG_SERVICE_URL"),
        std::env::var("CONFIG_SERVICE_NAMESPACE"),
    ) else {
        return std::collections::HashMap::new();
    };
    let endpoint = format!("{}/config/export/{}", url.trim_end_matches('/'), ns);
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::warn!(error = %e, "failed to build tokio runtime for remote config fetch");
            return std::collections::HashMap::new();
        }
    };
    rt.block_on(async {
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "failed to build reqwest client for remote config fetch");
                return std::collections::HashMap::new();
            }
        };
        match client.get(&endpoint).send().await {
            Ok(resp) if resp.status().is_success() => match resp
                .json::<std::collections::HashMap<String, String>>()
                .await
            {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(error = %e, url = %endpoint, "remote config json parse failed");
                    std::collections::HashMap::new()
                }
            },
            Ok(resp) => {
                tracing::warn!(status = %resp.status(), url = %endpoint, "remote config non-success");
                std::collections::HashMap::new()
            }
            Err(e) => {
                tracing::warn!(error = %e, url = %endpoint, "remote config fetch failed");
                std::collections::HashMap::new()
            }
        }
    })
}
