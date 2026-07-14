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
3. **Database** (MySQL / SQLite / both / none — drives migration setup)
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
version = "0.1.0"
edition = "2021"

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
utoipa = { version = "5", features = ["axum_extras", "chrono", "yaml"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
common-error = { path = "../../libs/common-error" }
common-db = { path = "../../libs/common-db", features = ["mysql", "sqlite"] }
common-log = { path = "../../libs/common-log" }
common-cache = { path = "../../libs/common-cache" }
common = { path = "../../libs/common" }
consul-registrar = { path = "../../libs/consul-registrar" }
# Optional OTel: uncomment if tracing is needed
# common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }

[[bin]]
name = "dump-openapi"
path = "src/bin/dump-openapi.rs"

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

## Dockerfile Template

```dockerfile
FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/ services/
COPY sdk sdk

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release --target x86_64-unknown-linux-musl -p {name} && \
    cp /app/target/x86_64-unknown-linux-musl/release/{name} /app/{name}

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/{name} /usr/local/bin/{name}
EXPOSE 8080
ENV RUST_LOG=info
CMD ["{name}"]
```

## src/main.rs Template

```rust
use {snake_name}::{config::AppConfig, routes, create_state};
use common_log::init_json;
use consul_registrar::ConsulRegistrar;
use std::net::SocketAddr;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_json("info");

    let config = AppConfig::from_env();
    info!("Starting {name} on {}", config.server_addr);

    // If using DB:
    let pool = common::db::connect(&config.database_url, 10)
        .await
        .expect("Failed to initialize database pool");

    let state = create_state(pool);
    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
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

    let mut consul_registrar = ConsulRegistrar::new(
        &config.consul_addr,
        &config.consul_service_name,
        "/tmp/{name}-id",
    );
    if let Err(e) = consul_registrar
        .register_discovered(addr.port(), &config.consul_service_name)
        .await
    {
        warn!("failed to register with consul: {}", e);
    }

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("failed to listen for ctrl-c");
        info!("received shutdown signal, deregistering from consul");
        let _ = consul_registrar.deregister().await;
        std::process::exit(0);
    });

    axum::serve(listener, app).await.expect("Server error");
    Ok(())
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
use serde::Deserialize;

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_log() -> String { "info".to_string() }
fn default_db_url() -> String { "mysql://root:rootpass@localhost:3306/{name}".to_string() }
fn default_consul() -> String { "http://localhost:8500".to_string() }

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_host")]
    pub server_host: String,
    #[serde(default = "default_port")]
    pub server_port: u16,
    pub server_addr: String,
    #[serde(default = "default_db_url")]
    pub database_url: String,
    #[serde(default = "default_log")]
    pub log_level: String,
    #[serde(default = "default_consul")]
    pub consul_addr: String,
    pub consul_service_name: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let server_host = std::env::var("SERVER_HOST").unwrap_or_else(|_| default_host());
        let server_port: u16 = std::env::var("SERVER_PORT")
            .ok().and_then(|v| v.parse().ok()).unwrap_or_else(default_port);
        let server_addr = std::env::var("SERVER_ADDR")
            .unwrap_or_else(|_| format!("{}:{}", server_host, server_port));

        Self {
            server_host,
            server_port,
            server_addr,
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(default_db_url),
            log_level: std::env::var("RUST_LOG").unwrap_or_else(default_log),
            consul_addr: std::env::var("CONSUL_ADDR").unwrap_or_else(default_consul),
            consul_service_name: std::env::var("CONSUL_SERVICE_NAME")
                .unwrap_or_else(|_| "{name}".to_string()),
        }
    }
}
```

## src/error.rs Template

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
        let body = json!({
            "code": code,
            "message": self.to_string(),
        });
        (status, Json(body)).into_response()
    }
}
```

## src/routes/mod.rs Template

```rust
use axum::{routing::get, Router};
use prometheus::{Encoder, TextEncoder};
use sqlx::MySqlPool;
use std::sync::Arc;

pub type AppState = Arc<AppServices>;

pub struct AppServices {
    pub pool: MySqlPool,
}

pub fn create_state(pool: MySqlPool) -> AppState {
    Arc::new(AppServices { pool })
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .with_state(state)
}

async fn health() -> &'static str { "OK" }

async fn metrics() -> axum::response::Html<String> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    axum::response::Html(String::from_utf8(buffer).unwrap())
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
# 1. Workspace member already added by Auto-Registration step 1.
#    Verify it's present: grep "services/{name}" Cargo.toml

# 2. Build (must use docker)
docker run --rm --network host \
  -v $(pwd):/app \
  -v coder-cargo-registry:/usr/local/cargo/registry \
  -v coder-cargo-git:/usr/local/cargo/git \
  -v coder-cargo-target:/app/target \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -w /app \
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl \
  cargo check --target x86_64-unknown-linux-musl -p {name}

# 3. Run tests
node scripts/run-tests-docker.js {name}

# 4. Service standard gate
node scripts/check-service-standard.js
# Expected: ✓ 8/8 items for {name}

# 5. Generate OpenAPI yaml
docker run --rm -v $(pwd):/app -v coder-cargo-target:/app/target \
  -w /app/services/{name} \
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl \
  cargo run --target x86_64-unknown-linux-musl --bin dump-openapi

# 6. OpenAPI spec check
node scripts/check-spec.js
# Expected: warnings (no summaries) but exit 0
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

Insert a new service block. Use the `x-svc-registrator-labels` anchor pattern
from existing services. Insert before the `volumes:` section (or before the
`# >>> business-services (auto-appended by skill) <<<` anchor if it exists):

```yaml
  # ─────────── {name} (Rust + Axum) ───────────
  {name}:
    build:
      context: ..
      dockerfile: services/{name}/Dockerfile
    ports:
      - "{external_port}:8080"
    labels:
      - "SERVICE_NAME={name}"
      - "SERVICE_ID={name}"
      - "SERVICE_PORT=8080"
      - "SERVICE_TAGS=backend,{name},rust"
      - "SERVICE_8080_CHECK_HTTP=/health"
      - "SERVICE_8080_CHECK_INTERVAL=15s"
      - "SERVICE_8080_CHECK_TIMEOUT=3s"
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
- ❌ Forgetting `register_discovered` for Consul → service won't be discoverable
- ❌ Missing `db.rs` schema bootstrap → service starts but tables don't exist
- ❌ Not copying `services/agents-template.md` → CI service-standard check fails on missing AGENTS.md

## Out of Scope

- Custom middleware (rate limiting, OTel tracing) — these are opt-in via env vars; default template has none
- Frontend integration — separate workflow, see `.trae/skills/coder-dev-bootstrap/`
- Production deployment configs — handled at infra layer, not service scaffold