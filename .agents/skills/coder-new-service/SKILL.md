---
name: coder-new-service
description: |
  Scaffold a brand-new Rust microservice following the 8-item service standard.
  Use when the user says "new service", "create a service", "scaffold X service",
  or any equivalent intent to add a Rust backend to the platform.
---

# coder-new-service Skill

Scaffold a new Rust microservice that satisfies the **8-item service standard baseline**
(ADR-002) on first commit — no exemptions required.

## Service Scope

Any new Rust service goes here. Existing 8 services cover: api-gateway, token-service,
iam-service, notification-service, audit-service, config-service, file-service,
prue_backend.

## Gathering Service Info

Ask the user for:

1. **Service name** (kebab-case, e.g. `analytics-service`, `billing-service`)
2. **Purpose** (one sentence — what business problem this service solves)
3. **Database** (MySQL / PostgreSQL / SQLite / none — drives optional dependency, pool, migration and health-check generation)
4. **Initial routes** (optional — empty service is fine for v1; routes added later via `coder-add-route`)
5. **Initial DTOs / Models** (optional)
6. **External service dependencies** (e.g. "calls token-service to validate JWT")
7. **Port** (default: 8080 inside container, mapped externally in docker-compose)

## File Scaffolding (10 files minimum)

### Always created

| File | Purpose |
|------|---------|
| `services/{name}/Cargo.toml` | Crate manifest with workspace deps |
| `services/{name}/Dockerfile` | Multi-stage musl build |
| `services/{name}/AGENTS.md` | Service conventions (copy from `services/agents-template.md`) |
| `services/{name}/README.md` | Quick-start + endpoint list |
| `services/{name}/src/main.rs` | Bootstrap (config + DB + routes + consul) |
| `services/{name}/src/lib.rs` | Module exports |
| `services/{name}/src/config.rs` | `AppConfig::from_env()` |
| `services/{name}/src/error.rs` | `AppError` + `AppResult` |
| `services/{name}/src/routes/mod.rs` | Router + /health + /metrics |
| `services/{name}/src/openapi.rs` | `ApiDoc` (utoipa) |

### If database is MySQL / SQLite / both

| File | Purpose |
|------|---------|
| `services/{name}/migrations/mysql/V001__init.sql` | Initial MySQL schema |
| `services/{name}/migrations/sqlite/V001__init.sql` | Initial SQLite schema (if both) |
| `services/{name}/src/db.rs` | Pool init + schema bootstrap |

### If routes are needed at scaffold time

| File | Purpose |
|------|---------|
| `services/{name}/src/routes/{feature}.rs` | First route handler with `#[utoipa::path]` |
| `services/{name}/tests/{feature}_test.rs` | Smoke test |
| `services/{name}/src/bin/dump-openapi.rs` | OpenAPI yaml generator |

### Always created (config)

| File | Purpose |
|------|---------|
| `services/{name}/config/config.yaml` | Default config (gitignored in some setups, kept here for dev) |

## Cargo.toml Template

```toml
[package]
name = "{name}"
version.workspace = true
edition.workspace = true

[dependencies]
anyhow = "1"
axum.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
prometheus = "0.13"
# DB-backed MySQL example only; omit sqlx and digua_common_db for no-DB services.
sqlx = { workspace = true, features = ["mysql"] }
utoipa = { version = "5", features = ["axum_extras", "chrono", "yaml"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
# 平台基础设施 crate 已发布到 crates.io；不要依赖私有 Kellnr registry。
# 版本必须与 crates.io 当前发布版本一致；当前仓库基线为 1.6.0。
digua_common_error = "1.6.0"
digua_common_db = { version = "1.6.0", default-features = false, features = ["mysql"] }
digua_common_log = "1.6.0"
digua_common_cache = "1.6.0"
digua_common = { version = "1.6.0", default-features = false }
# 按需添加数据库 feature：mysql / postgres / sqlite；无数据库服务不添加 common-db。
# OTel opt-in，声明时必须显式开启 otlp（resolver=2）。
digua_common_telemetry = { version = "1.6.0", features = ["otlp"] }

[[bin]]
name = "dump-openapi"
path = "src/bin/dump-openapi.rs"

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

> **Registry**：基础设施 crate 从 crates.io 下载，不需要 Kellnr、私有 token 或仓库专属 `.cargo` source 配置。若组织使用镜像，应由开发者本机/CI 的 Cargo 全局配置提供，不要把私有 registry 地址或凭据写进模板。

## Dockerfile Template

```dockerfile
FROM rust:1.96-slim AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY .cargo/ .cargo/
COPY libs/ libs/
COPY services/ services/
COPY sdk/ sdk/

# 只使用公开 crates.io 依赖；不要把 registry token 通过 ARG/ENV 传入镜像构建。
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release -p {name} && \
    cp /app/target/release/{name} /app/{name}

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/{name} /usr/local/bin/{name}
EXPOSE 8080
ENV RUST_LOG=info
CMD ["{name}"]
```

> **构建命令**：
> ```bash
> docker build -f services/{name}/Dockerfile -t {name}:latest .
> ```

## src/main.rs Template

```rust
use {snake_name}::{config::AppConfig, routes, create_state};
use digua_common_log::init_json;
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_json("info");

    let config = AppConfig::try_from_env()?;
    info!("Starting {name} on {}", config.server_addr);

    // This sample is the MySQL-backed variant. Generate a no-DB state constructor
    // and omit sqlx/common-db entirely when the service has no database.
    let state = create_state(&config).await?;
    let addr: SocketAddr = config.server_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("{name} listening on {}", addr);

    let app = routes::routes(state);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false);
    let app = if swagger_enabled {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        info!("swagger ui enabled at /swagger-ui");
        app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", {snake_name}::openapi::ApiDoc::openapi()),
        )
    } else { app };

    // Consul self-registration: digua_common_consul::consul_guard() reads the
    // SERVICE_* environment variables (set in docker-compose-services.yml),
    // registers this instance at startup, deregisters on SIGTERM, and
    // periodically re-registers if the Consul catalog loses it.
    let _consul_guard = digua_common_consul::consul_guard();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal;
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = signal::ctrl_c() => {},
        _ = terminate => {},
    }
}
```

## src/lib.rs Template

```rust
pub mod config;
pub mod error;
pub mod openapi;
pub mod routes;

pub use config::AppConfig;
pub use error::{AppError, AppResult};
pub use routes::{create_state, routes};
```

## src/config.rs Template

```rust
fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_log() -> String { "info".to_string() }

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server_host: String,
    pub server_port: u16,
    pub server_addr: String,
    // Keep this field only for services that declare a database.
    pub database_url: Option<String>,
    pub log_level: String,
}

impl AppConfig {
    pub fn try_from_env() -> anyhow::Result<Self> {
        let server_host = std::env::var("SERVER_HOST").unwrap_or_else(|_| default_host());
        let server_port = std::env::var("SERVER_PORT")
            .map(|value| value.parse::<u16>())
            .transpose()?
            .unwrap_or_else(default_port);
        let server_addr = std::env::var("SERVER_ADDR")
            .unwrap_or_else(|_| format!("{server_host}:{server_port}"));

        // A DB-backed service must reject a missing DATABASE_URL at startup.
        // A no-DB service should omit database_url and the sqlx dependency entirely.
        let database_url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());

        server_addr.parse::<std::net::SocketAddr>()?;
        Ok(Self {
            server_host,
            server_port,
            server_addr,
            database_url,
            log_level: std::env::var("RUST_LOG").unwrap_or_else(|_| default_log()),
        })
    }
}
```

> `try_from_env()` 返回 `anyhow::Result<Self>`。不要提供默认数据库密码；数据库服务必须显式要求 `DATABASE_URL`，无数据库服务应移除数据库字段与依赖。
 
## src/error.rs Template (DB-backed example; no-DB services should omit the Database variant and sqlx dependency)

```rust
use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
        };
        let public_message = match &self {
        AppError::NotFound => "Resource not found",
        AppError::BadRequest(_) => "Invalid request",
        AppError::Database(_) | AppError::Internal(_) => "Internal server error",
    };
    let body = json!({
            "code": code,
            "message": public_message,
        });
        (status, Json(body)).into_response()
    }
}
```

## src/routes/mod.rs Template (MySQL example; generate dialect-specific or no-DB state when selected)

```rust
use axum::{extract::State, response::IntoResponse, routing::get, Router};
use anyhow::Context;
use prometheus::{Encoder, TextEncoder};
use sqlx::MySqlPool;
use std::sync::Arc;

pub type AppState = Arc<AppServices>;

pub struct AppServices {
    pub pool: MySqlPool,
}

pub async fn create_state(config: &crate::config::AppConfig) -> anyhow::Result<AppState> {
    let database_url = config.database_url.as_deref().context("DATABASE_URL is required for this DB-backed service")?;
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(Arc::new(AppServices { pool }))
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(ready)) // compatibility endpoint used by Consul and service-standard checks
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/metrics", get(metrics))
        .with_state(state)
}

async fn live() -> &'static str { "OK" }
// readiness must check dependencies actually required to serve traffic (e.g. DB ping).
async fn ready(State(state): State<AppState>) -> axum::http::StatusCode {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => axum::http::StatusCode::OK,
        Err(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn metrics() -> impl axum::response::IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    if encoder.encode(&metric_families, &mut buffer).is_err() {
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "metrics encoding failed".to_string()).into_response();
    }
    ([(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], String::from_utf8_lossy(&buffer).into_owned()).into_response()
}
```

## src/openapi.rs Template

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "{name}", description = "{purpose}"),
    paths(),
    components(schemas()),
    tags()
)]
pub struct ApiDoc;
```

## AGENTS.md Customization

After scaffolding, customize the AGENTS.md (copied from `services/agents-template.md`):

- Replace `<service-name>` with `{name}`
- Replace environment variable defaults to match `src/config.rs`
- List endpoint domain (start with `/health`, `/metrics`, `/swagger-ui`)
- Add any exemptions if needed (e.g. "no migrations" if no DB)

## Validation (run after scaffolding)

```bash
# 1. Confirm workspace registration
grep "services/{name}" Cargo.toml

# 2. Build and test using the repository's supported Rust toolchain
cargo check -p {name}
cargo test -p {name}

# 3. Service standard and OpenAPI gates
node scripts/check-service-standard.js
node scripts/check-spec.js

# 4. If the repository's Docker builder is available, build the service image
docker build -f services/{name}/Dockerfile -t {name}:latest .
```

## Auto-Registration (3 append operations)

After creating all files, the skill MUST perform these 3 registrations so the
service compiles in the workspace and is reachable via docker-compose + gateway.
Do not leave the user to do them manually.

### 1. Add to root `Cargo.toml` workspace members

Edit the root `Cargo.toml` `[workspace]` section and add the new service to the
`members` array (Cargo does NOT support globs - each service must be listed):

```toml
[workspace]
members = [
    # ... existing members ...
    "services/{name}",
]
```

Place the new entry after the last `services/` entry to keep alphabetical-ish
ordering. Without this, `cargo check -p {name}` and `cargo build -p {name}`
will fail with "package not found in workspace".

### 2. Append to `infrastructure/docker-compose-services.yml`

Insert a new service block following the existing pattern (service
registration is by env vars + `digua_common_consul` self-registration — no
Docker labels). Insert before the `volumes:` section:

```yaml
  # ─────────── {name} (Rust + Axum) ───────────
  {name}:
    build:
      context: ..
      dockerfile: services/{name}/Dockerfile
    ports:
      - "{external_port}:8080"
    environment:
      SERVICE_BOOTSTRAP_TOKEN: "${{name-upper}_DEV_BT}"
      TENANT_ID: "${TENANT_ID}"
      TOKEN_SERVICE_URL: "${TOKEN_SERVICE_URL}"
      JWKS_URL: "${JWKS_URL}"
      SERVER_HOST: "0.0.0.0"
      SERVER_PORT: "8080"
      DATABASE_URL: "${DATABASE_URL}"
      CONSUL_ADDR: "http://consul:8500"
      RUST_LOG: "info"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
      SERVICE_NAME: "{name}"
      SERVICE_ID: "{name}"
      SERVICE_PORT: "8080"
      SERVICE_HEALTH_PATH: "/health"
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
    networks:
      - superpowers
```

> `{name-upper}` = `{name}` in uppercase with hyphens replaced by underscores
> (e.g. `billing-service` -> `BILLING_SERVICE`).
> `{external_port}` = pick the next free port (existing services use 8081-8087
> for docker, 6001-6007 for host; check `infrastructure/README.md` port table).
> If the service needs Redis or RabbitMQ, add them to `depends_on`.

### 3. Create api-gateway route file

Create `infrastructure/config/api-gateway/routes/{name}.router.toml` so
api-gateway forwards `/v1/{name}/*` to the new service:

```toml
[[routes]]
upstream = "/v1/{name}/*"
downstream = "http://{name}:8080/{name}/*"
service = "{name}"
public = false
ws = false
```

Verify: `cargo run -p api-gateway -- validate --routes infrastructure/config/api-gateway/routes`

### 4. (Optional) Add to `infrastructure/.env`

Add the bootstrap token and port export:
```
{name-upper}_DEV_BT=dev-bypass-token
```

## CI Verification Checklist

- [ ] `node scripts/check-service-standard.js` exits 0 with ✓ on new service
- [ ] `node scripts/check-spec.js` exits 0 (warnings OK)
- [ ] `node scripts/contracts-gen.js --check` exits 0
- [ ] `node scripts/run-tests-docker.js {name}` all tests pass
- [ ] AGENTS.md declares all 8 items
- [ ] Dockerfile builds successfully in Docker

## Common Mistakes to Avoid

- ❌ Forgetting to add the service to root `Cargo.toml` members → `cargo check` won't find it
- ❌ Using MySQL-specific types in SQLite migration → use port-appropriate types (see `coder-migrate-db` skill)
- ❌ Hardcoding port 3000 (that's api-gateway's port) → use 8080 internal, map externally
- ❌ Reintroducing private registry URLs or passing registry credentials through Docker build args
- ❌ Missing `db.rs` schema bootstrap → service starts but tables don't exist
- ❌ Not copying `services/agents-template.md` → CI service-standard check fails on missing AGENTS.md

## Out of Scope

- Custom middleware (rate limiting, OTel tracing) — these are opt-in via env vars; default template has none
- Frontend integration — separate workflow, see `.trae/skills/coder-dev-bootstrap/`
- Production deployment configs — handled at infra layer, not service scaffold