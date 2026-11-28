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

    // Phase 1 (Phase 5-B 演进): 把 bootstrap token seed 到 token_bootstrap_tokens 表。
    // 优先级：env CSV（SERVICE_BOOTSTRAP_TOKENS + SERVICE_BOOTSTRAP_OWNERS）→ 内置 dev 默认值。
    // 语义：按 owner 补插（已有的 owner 不覆盖），因此幂等且不破坏已 rotate 的 token。
    let seeded_bt = token_service::models::bootstrap_token::seed_defaults(
        &db,
        std::env::var("SERVICE_BOOTSTRAP_TOKENS")
            .unwrap_or_default()
            .as_str(),
        std::env::var("SERVICE_BOOTSTRAP_OWNERS")
            .unwrap_or_default()
            .as_str(),
    )
    .await
    .expect("seed bootstrap tokens");
    if seeded_bt > 0 {
        tracing::info!("seeded {} bootstrap tokens", seeded_bt);
    }

    // WP-LMMS-SERVICE-JWT-AUDIENCE-02: 内置 owner→audience 系统授权。
    // 顺序不变量：必须在 HTTP listener 接受请求之前完成——否则会出现
    // "端点已就绪但授权表未就绪"的窗口，IAM 登录链会在该窗口拿到 403。
    // 与 bootstrap token seed 解耦：无论密钥来自 env 注入还是 dev 默认值，
    // 系统 grant 都必须 seed（生产密钥注入不得抑制授权引导）。
    // fail-closed：任何一步失败 → 进程退出，listener 永不 bind。
    let seeded_grants = token_service::models::service_jwt_grant::ensure_system_grants(&db)
        .await
        .unwrap_or_else(|e| {
            eprintln!("ensure system service-jwt grants failed (fail-closed): {e}");
            std::process::exit(1);
        });
    if seeded_grants > 0 {
        tracing::info!("seeded {} built-in service-jwt grants", seeded_grants);
    }
    token_service::models::service_jwt_grant::validate_required_principals(&db)
        .await
        .unwrap_or_else(|e| {
            // 诊断信息不含任何 secret 材料。
            eprintln!("service-jwt grant principal validation failed (fail-closed): {e}");
            std::process::exit(1);
        });

    // ── JWT signing keystore (2026-11-23 persistence design) ──
    // Single source of truth on disk: keystore.json. Fail-closed on any
    // corruption — startup NEVER silently regenerates over an existing key.
    // Legacy deployments: an existing private.pem/public.pem pair is
    // imported once (kid stays "default") so old tokens keep verifying.
    let keystore_path = keystore_path_from_config(&cfg);
    let keystore = token_service::keystore::KeyStore::init(
        &keystore_path,
        &cfg.jwt_private_key_path,
        &cfg.jwt_public_key_path,
        cfg.jwt_min_retention_sec,
    )
    .unwrap_or_else(|e| {
        // No key material is ever logged — only the failure reason.
        eprintln!("JWT keystore init failed (fail-closed): {e}");
        std::process::exit(1);
    });

    let active_private_pem = keystore.active().private_pem.clone();
    let active_kid = keystore.active().kid.clone();
    let jwks_entries: Vec<(String, String)> = keystore
        .public_entries()
        .into_iter()
        .map(|k| (k.kid.clone(), k.public_pem.clone()))
        .collect();

    let issuer = Arc::new(
        TokenIssuer::with_key(&active_private_pem, &active_kid, &cfg.issuer)
            .expect("failed to create token issuer"),
    );

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    // We are the JWKS source. Self-loopback via refresh() would race with
    // axum::serve binding (the listener isn't accepting yet) and leave the
    // cache empty on every restart, breaking inbound service-jwt verify.
    // Instead of from_pem (single hard-coded "default" kid), install every
    // JWKS-published key (active + retiring) under its real kid so inbound
    // service-jwt verification is kid-consistent from the first request.
    let jwks_cache: std::sync::Arc<digua_common_jwt::JWKSCache> = std::sync::Arc::new(
        digua_common_jwt::JWKSCache::new(Default::default(), reqwest::Client::new()),
    );
    jwks_cache
        .install_from_pems(&jwks_entries)
        .await
        .expect("install initial JWKS keys from keystore");

    let state = TokenServerState {
        db: db.clone(),
        issuer,
        issuer_str: cfg.issuer.clone(),
        default_resource_ttl_sec: cfg.default_resource_ttl_sec,
        http_client: Arc::new(ResilientClient::new(
            reqwest::Client::new(),
            ResilienceConfig::default(),
            Arc::new(NoopObserver),
        )),
        user_service_url: cfg.user_service_url.clone(),
        keys: Arc::new(std::sync::RwLock::new(keystore)),
        jwks: jwks_cache,
        service_jwt_grant_admins: Arc::new(cfg.service_jwt_grant_admins.clone()),
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
    // /admin/keys* — JWT 签名密钥生命周期（rotate/retire/delete/list），
    // 同样 service-jwt 准入；rotate 即可冒充任意用户，绝不能裸暴露。
    app = app.merge(routes::keys_admin::routes(state.clone()));

    // Phase 3: /admin/bootstrap-tokens* —— bootstrap token 的管理面
    // （rotate / 禁用 / 启用 / 查询）。同样带 service-jwt 鉴权（aud=token-service）：
    // 能 rotate 就等于能冒充任意服务。
    app = app.merge(routes::bootstrap_token_admin::routes(state.clone()));

    // Phase 3 (web): /v1/admin/bootstrap-tokens* —— 同一份业务逻辑，但走
    // user-jwt + 权限码鉴权，供浏览器端 Web UI 调用（经 api-gateway 的
    // token.router.toml 转发）。service-jwt 端点保留给 CI / 运维脚本。
    app = app.merge(routes::bootstrap_token_web::routes(state.clone()));

    // WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01: grant 管理面（owner → audience
    // 授权关系的 CRUD + 审计，双入口共用 ops 业务层）。上游 WP-02 只建了
    // 运行时检查与 code-owned seed，管理能力由本任务补齐。
    app = app.merge(routes::service_jwt_grant_admin::routes(state.clone()));
    app = app.merge(routes::service_jwt_grant_web::routes(state.clone()));

    // Phase 5-B: service-jwt endpoint (bootstrap-token auth).
    app = app.merge(token_service::service_jwt_routes(state.clone()));

    // WP-LMMS-HARDEN-01-R1: /tokens/resource 曾是签发类端点中唯一无认证的
    // （公开签名神谕：任何调用方可让平台密钥签出任意 resource/permission JWT）。
    // 现要求 service-jwt（aud=token-service）准入，与 /tokens/user-jwt 一致。
    app = app.merge(routes::resource::routes(state.clone()));

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

    // WP-BS-01 / WP-01D: /internal/bootstrap/* —— IAM Bootstrap 流程的
    // provisioning 调用面（readiness / ensure-grants）。鉴权与其他内部
    // 端点一致：service-jwt（aud=token-service）。
    app = app.merge(routes::internal_bootstrap::routes(state.clone()).layer(
        axum::middleware::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<token_service::TokenServerState>(
                "token-service",
            ),
        ),
    ));

    // 自注册（libs/common-consul）：SERVICE_NAME 未设时为 no-op。
    let consul_guard = digua_common_consul::consul_guard();
    tokio::spawn(async move {
        // SIGINT + SIGTERM（docker stop 走 SIGTERM；旧代码只听 ctrl_c，
        // SIGTERM 无人消费 → 超时 SIGKILL，注销路径从未真正执行）。
        digua_common_consul::wait_for_shutdown_signal().await;
        tracing::info!("received shutdown signal, deregistering from consul");
        consul_guard.deregister().await;
        std::process::exit(0);
    });

    tracing::info!("token-service listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

/// keystore.json lives next to the private key by default (containers:
/// `/app/keys/keystore.json`), overridable via `TOKEN_JWT_KEYSTORE_PATH`.
fn keystore_path_from_config(cfg: &token_service::config::Config) -> std::path::PathBuf {
    if cfg.jwt_keystore_path.trim().is_empty() {
        std::path::Path::new(&cfg.jwt_private_key_path)
            .parent()
            .map(|p| p.join("keystore.json"))
            .unwrap_or_else(|| std::path::PathBuf::from("keystore.json"))
    } else {
        std::path::PathBuf::from(&cfg.jwt_keystore_path)
    }
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
