# P1.3 Service Standard Sweep Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 8 个服务对齐到统一规范基线（必需组件 + 端点 + 环境变量 + Swagger 开关），并把 `check:service-standard` / `check:spec` / `contracts:check` 三项作为 CI 强制闸门。

**Architecture:** 不改业务运行时；仅调整：(1) 各服务 `AGENTS.md` 与端点注册（补 `/health` `/metrics` 和 `SWAGGER_ENABLED` 开关），(2) `scripts/check-service-standard.js` 规则集与 exit code，(3) `scripts/contracts-gen.js` 增加 `--check` 子命令，(4) `.gitlab-ci.yml` 串联三项闸门，(5) `services/agents-template.md` 升级为完整模板，(6) ADR-002 固化基线。

**Tech Stack:** Rust (axum 0.8 + utoipa 5 + utoipa-swagger-ui 9 + prometheus 0.13) / Node 20 / GitLab CI / Docker。

## Global Constraints

- 不改任何对外 HTTP 接口行为；只新增 `/health` `/metrics` `/swagger-ui` 三类系统端点。
- Swagger 默认关闭（`SWAGGER_ENABLED=false`），与 audit/config/file/prue 当前默认行为一致；显式 `=true` 才挂载。
- `/health` 返回 `"OK"` 纯文本，`/metrics` 返回 Prometheus 文本格式（沿用 iam/file/prue 现有 handler 模式）。
- 每服务单独 commit，按顺序：audit-service → config-service → file-service → prue_backend → notification-service → token-service → iam-service → api-gateway。
- 所有 Cargo 依赖 `utoipa = "5"`，`utoipa-swagger-ui = "9"`，`prometheus = "0.13"`（与现有 iam/notification 对齐）。
- 任何 `AGENTS.md` 必须包含三段：`## 必需组件` / `## 环境变量` / `## 端点域`；豁免必须在 `## 豁免` 段显式声明。
- `scripts/check-service-standard.js` 在发现 warning 时必须 `process.exit(1)`。
- 工作流保持 `agents.md` 规则：网络访问 GitHub 必须 `gh-proxy.org`；本地开发用 WSL/macOS/Linux。

---

## File Structure

**调整脚本：**
- Modify: `scripts/check-service-standard.js` — 升级规则（8 项 + 豁免段 + exit code）
- Modify: `scripts/contracts-gen.js` — 新增 `--check` 子命令
- Modify: `package.json` — 已有 `contracts:check`，确认行为正确（无改动）

**调整规范文档：**
- Modify: `services/agents-template.md` — 升级为完整模板
- Modify: `services/api-gateway/agents.md` → 重命名 `AGENTS.md`，对齐模板
- Modify: `services/token-service/AGENTS.md` — 对齐模板
- Modify: `services/iam-service/AGENTS.md` — 对齐模板（保留端点表）
- Modify: `services/notification-service/AGENTS.md` — 对齐模板
- Modify: `services/audit-service/AGENTS.md` — 对齐模板
- Modify: `services/config-service/AGENTS.md` — 对齐模板
- Modify: `services/file-service/AGENTS.md` — 对齐模板
- Modify: `services/prue_backend/AGENTS.md` — 对齐模板（保留端点表）

**调整服务源码：**
- Modify: `services/audit-service/src/routes/mod.rs` — 补 `/metrics`
- Modify: `services/audit-service/Cargo.toml` — 加 prometheus / utoipa
- Modify: `services/audit-service/src/main.rs` — 加 `SWAGGER_ENABLED` 开关
- Modify: `services/audit-service/src/lib.rs` — 暴露 `openapi::ApiDoc`
- Create: `services/audit-service/src/openapi.rs` — utoipa ApiDoc 占位

- Modify: `services/config-service/src/routes/mod.rs` — 补 `/health` `/metrics`
- Modify: `services/config-service/Cargo.toml` — 加 prometheus / utoipa
- Modify: `services/config-service/src/main.rs` — 加 `SWAGGER_ENABLED` 开关
- Modify: `services/config-service/src/lib.rs` — 暴露 `openapi::ApiDoc`
- Create: `services/config-service/src/openapi.rs` — utoipa ApiDoc 占位

- Modify: `services/file-service/Cargo.toml` — 加 utoipa
- Modify: `services/file-service/src/main.rs` — 加 `SWAGGER_ENABLED` 开关
- Modify: `services/file-service/src/lib.rs` — 暴露 `openapi::ApiDoc`
- Create: `services/file-service/src/openapi.rs` — utoipa ApiDoc 占位

- Modify: `services/prue_backend/Cargo.toml` — 加 utoipa
- Modify: `services/prue_backend/src/main.rs` — 加 `SWAGGER_ENABLED` 开关
- Modify: `services/prue_backend/src/lib.rs` — 暴露 `openapi::ApiDoc`
- Create: `services/prue_backend/src/openapi.rs` — utoipa ApiDoc 占位

**CI / .gitignore：**
- Modify: `.gitlab-ci.yml` — service_standard_check 之后追加 `check:spec` + `contracts:check` 两个 job
- Modify: `.gitignore` — 新增 `/build-*.log`
- Delete: `build-gw.log`、`build-iam.log`（根目录）

**ADR：**
- Create: `docs-site/decisions/ADR-002-service-standard-baseline.md`

---

## Task 1: 升级 `scripts/check-service-standard.js` 与 `services/agents-template.md`

**Files:**
- Modify: `services/agents-template.md`
- Modify: `scripts/check-service-standard.js`

**Interfaces:**
- Produces: 规则集（8 项 + 豁免段 + exit code 1），后续任务依赖此脚本判定通过

- [ ] **Step 1.1: 升级模板** — 覆盖 `services/agents-template.md`：

```markdown
# <service-name> 服务规范

## 必需组件

- [ ] Dockerfile
- [ ] AGENTS.md
- [ ] README.md
- [ ] tests/ 目录
- [ ] /health 端点
- [ ] /metrics 端点
- [ ] migrations/ 目录
- [ ] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址（优先级高于上两项）|
| DATABASE_URL | 服务自定义 | 数据库连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |
| CONSUL_ADDR | 服务自定义 | Consul 地址 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true 时启用）|

## 豁免

（无豁免则保留空段；豁免项写在此处，每行一项必需组件名）

## AI 检查时机

修改 services/<name>/ 下文件时自动检查
```

- [ ] **Step 1.2: 升级 `scripts/check-service-standard.js`** — 覆盖文件：

```javascript
#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');

const SERVICES_DIR = path.join(__dirname, '..', 'services');
const REQUIRED = [
  'Dockerfile',
  'AGENTS.md',
  'README.md',
  'tests/ 目录',
  '/health 端点',
  '/metrics 端点',
  'migrations/ 目录',
  'Swagger 配置（SWAGGER_ENABLED 开关）',
];

function parseExemptions(content) {
  const m = content.match(/## 豁免\s*\n([\s\S]*?)(?:\n##|$)/);
  if (!m) return new Set();
  return new Set(
    m[1]
      .split('\n')
      .map(l => l.replace(/^[-*]\s*/, '').trim())
      .filter(Boolean)
  );
}

function fileExists(p) {
  return fs.existsSync(p);
}

function readSafe(p) {
  return fileExists(p) ? fs.readFileSync(p, 'utf-8') : '';
}

function hasRoute(serviceDir, route) {
  const candidates = [
    path.join(serviceDir, 'src/main.rs'),
    path.join(serviceDir, 'src/routes/mod.rs'),
    path.join(serviceDir, 'src/lib.rs'),
  ];
  const regex = new RegExp(`\\.route\\(\\s*"${route}"`);
  return candidates.some(p => regex.test(readSafe(p)));
}

function hasSwagger(serviceDir) {
  const cargo = readSafe(path.join(serviceDir, 'Cargo.toml'));
  if (!/utoipa-swagger-ui/.test(cargo)) return false;
  const main = readSafe(path.join(serviceDir, 'src/main.rs'));
  return /SWAGGER_ENABLED/.test(main);
}

function checkComponent(serviceDir, component) {
  switch (component) {
    case 'Dockerfile':
      return fileExists(path.join(serviceDir, 'Dockerfile'));
    case 'AGENTS.md':
      return fileExists(path.join(serviceDir, 'AGENTS.md'));
    case 'README.md':
      return fileExists(path.join(serviceDir, 'README.md'));
    case 'tests/ 目录':
      return (
        fileExists(path.join(serviceDir, 'tests')) &&
        fs.readdirSync(path.join(serviceDir, 'tests')).some(f => f.endsWith('.rs'))
      );
    case '/health 端点':
      return hasRoute(serviceDir, '/health');
    case '/metrics 端点':
      return hasRoute(serviceDir, '/metrics');
    case 'migrations/ 目录':
      return fileExists(path.join(serviceDir, 'migrations'));
    case 'Swagger 配置（SWAGGER_ENABLED 开关）':
      return hasSwagger(serviceDir);
    default:
      return false;
  }
}

function main() {
  const services = fs
    .readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(d => d.isDirectory())
    .map(d => d.name);

  console.log('=== 服务规范检查报告 ===\n');
  let failed = false;

  for (const svc of services) {
    const svcDir = path.join(SERVICES_DIR, svc);
    const agentsPath = path.join(svcDir, 'AGENTS.md');
    console.log(`\n## ${svc}`);

    if (!fileExists(agentsPath)) {
      console.log('⚠️  缺失规则文件: AGENTS.md');
      failed = true;
      continue;
    }

    const exemptions = parseExemptions(fs.readFileSync(agentsPath, 'utf-8'));

    for (const comp of REQUIRED) {
      if (exemptions.has(comp)) {
        console.log(`◯ 豁免: ${comp}`);
        continue;
      }
      if (checkComponent(svcDir, comp)) {
        console.log(`✓ ${comp}`);
      } else {
        console.log(`⚠️  缺失: ${comp}`);
        failed = true;
      }
    }
  }

  if (failed) {
    console.log('\n❌ 服务规范检查未通过');
    process.exit(1);
  }
  console.log('\n✅ 服务规范检查通过');
}

main();
```

- [ ] **Step 1.3: 跑脚本** — 当前仓库会暴露已知缺口（audit /metrics、config /health+/metrics、4 服务 Swagger、api-gateway 文件名等）。

Run: `node scripts/check-service-standard.js`
Expected: exit 1，输出含多条 `⚠️  缺失:`；这是符合预期的当前差距快照，作为本计划起点。

- [ ] **Step 1.4: 提交**

```bash
git add services/agents-template.md scripts/check-service-standard.js
git commit -m "feat(check-service-standard): 8-item rules + exemption + exit code"
```

---

## Task 2: audit-service 规范收口

**Files:**
- Modify: `services/audit-service/Cargo.toml`
- Modify: `services/audit-service/src/routes/mod.rs`
- Modify: `services/audit-service/src/lib.rs`
- Modify: `services/audit-service/src/main.rs`
- Create: `services/audit-service/src/openapi.rs`
- Modify: `services/audit-service/AGENTS.md`

**Interfaces:**
- Consumes: Task 1 的脚本规则
- Produces: audit-service 通过 8 项检查

- [ ] **Step 2.1: 加 Cargo 依赖**

替换 `services/audit-service/Cargo.toml` 的 `[dependencies]` 段（保留原有项，追加 4 项）：

```toml
[dependencies]
anyhow = "1"
axum.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
sqlx.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
csv.workspace = true
prometheus = "0.13"
utoipa = { version = "5", features = ["axum_extras", "chrono"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
digua_common-error = { path = "../../libs/digua_common-error" }
digua_common-db = { path = "../../libs/digua_common-db", features = ["mysql", "sqlite"] }
digua_common-log = { path = "../../libs/digua_common-log" }
digua_common-cache = { path = "../../libs/digua_common-cache" }
digua_common = { path = "../../libs/digua_common" }
consul-registrar = { path = "../../libs/consul-registrar" }
```

- [ ] **Step 2.2: 新建 `services/audit-service/src/openapi.rs`** — 内容：

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "audit-service", description = "Audit log and retention"),
    tags(
        (name = "audit", description = "Audit logs"),
    )
)]
pub struct ApiDoc;
```

- [ ] **Step 2.3: 在 `services/audit-service/src/lib.rs` 顶部加** `pub mod openapi;`

Use Edit to add after the existing `pub mod` declarations. If lib.rs doesn't currently declare modules, add this line near the top. Verify by `grep -n "pub mod" services/audit-service/src/lib.rs` — expect `openapi` to appear.

- [ ] **Step 2.4: 升级 `services/audit-service/src/routes/mod.rs`** — 加 `/metrics`：

```rust
pub mod compliance;
pub mod operation;
pub mod login;
pub mod retention;

use std::sync::Arc;
use crate::services::{AuditService, CleanupService};
use crate::models::RetentionPolicy;
use crate::error::AppResult;
use axum::{Router, routing::get};
use sqlx::mysql::MySqlPool;
use prometheus::{Encoder, TextEncoder};

pub type AppState = Arc<AppServices>;

pub struct AppServices {
    pub audit: AuditService,
    pub cleanup: CleanupService,
}

impl AppServices {
    pub async fn new(pool: MySqlPool) -> AppResult<Self> {
        let policy = RetentionPolicy::default();
        let audit = AuditService::new(&pool);
        let cleanup = CleanupService::new(&pool, policy);

        Ok(Self { audit, cleanup })
    }

    pub async fn get_operation_logs_by_filter(
        &self,
        user_id: Option<&str>,
        action: Option<&str>,
        resource: Option<&str>,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> AppResult<Vec<crate::models::OperationLog>> {
        self.audit
            .query_logs(user_id, action, resource, start_date, end_date, 10000, 0)
            .await
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/audit", operation::routes(state.clone()))
        .nest("/audit", login::routes(state.clone()))
        .nest("/audit", compliance::routes(state.clone()))
        .nest("/audit", retention::routes(state.clone()))
        .route("/health", get(health))
        .route("/audit/health", get(health))
        .route("/metrics", get(metrics))
}

async fn health() -> &'static str {
    "OK"
}

async fn metrics() -> axum::response::Html<String> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    axum::response::Html(String::from_utf8(buffer).unwrap())
}

pub fn create_state(pool: MySqlPool) -> AppState {
    Arc::new(AppServices {
        audit: AuditService::new(&pool),
        cleanup: CleanupService::new(&pool, RetentionPolicy::default()),
    })
}
```

- [ ] **Step 2.5: 升级 `services/audit-service/src/main.rs`** — 接 `SWAGGER_ENABLED`：

替换原 `let app = routes::routes(state);` 那一行及之后到 `axum::serve` 的片段为：

```rust
    let mut app = routes::routes(state);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false);
    if swagger_enabled {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", audit_service::openapi::ApiDoc::openapi()),
        );
        info!("swagger ui enabled at /swagger-ui");
    }
```

注：保留原 main.rs 中所有 consul / shutdown / serve 逻辑不变；仅替换 `let app = routes::routes(state);` 这一行为上述代码块。

- [ ] **Step 2.6: 重写 `services/audit-service/AGENTS.md`** — 用统一模板：

```markdown
# audit-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | MySQL 连接串 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /audit/operation-logs | GET / POST | 操作审计 |
| /audit/login-logs | GET / POST | 登录审计 |
| /audit/compliance/* | GET | 合规导出 |
| /audit/retention/* | GET / PUT | 留存策略 |
| /audit/health | GET | 业务健康 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 豁免

（无）

## AI 检查时机

修改 services/audit-service/ 下文件时自动检查
```

- [ ] **Step 2.7: 编译与跑** — 验证：

```bash
node scripts/run-tests-docker.js audit-service
```

Expected: PASS（容器内 `cargo test -p audit-service` 退出 0）

- [ ] **Step 2.8: 跑规范脚本**

```bash
node scripts/check-service-standard.js
```

Expected: audit-service 8 项全 ✓（其它服务依然有 ⚠️，脚本仍 exit 1，符合本任务局部验证目标）

- [ ] **Step 2.9: 提交**

```bash
git add services/audit-service
git commit -m "feat(audit-service): add /metrics, SWAGGER_ENABLED switch, align AGENTS.md"
```

---

## Task 3: config-service 规范收口

**Files:**
- Modify: `services/config-service/Cargo.toml`
- Modify: `services/config-service/src/routes/mod.rs`
- Modify: `services/config-service/src/lib.rs`
- Modify: `services/config-service/src/main.rs`
- Create: `services/config-service/src/openapi.rs`
- Modify: `services/config-service/AGENTS.md`

**Interfaces:**
- Consumes: Task 1 脚本规则
- Produces: config-service 通过 8 项检查

- [ ] **Step 3.1: 加 Cargo 依赖** — 追加 prometheus / utoipa：

```toml
[package]
name = "config-service"
version = "0.1.0"
edition = "2021"

[dependencies]
axum.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
sqlx.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
prometheus = "0.13"
utoipa = { version = "5", features = ["axum_extras", "chrono"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
digua_common-error = { path = "../../libs/digua_common-error" }
digua_common-db = { path = "../../libs/digua_common-db", features = ["mysql", "sqlite"] }
digua_common-log = { path = "../../libs/digua_common-log" }
digua_common-cache = { path = "../../libs/digua_common-cache" }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 3.2: 新建 `services/config-service/src/openapi.rs`**

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "config-service", description = "Dynamic configuration management"),
    tags(
        (name = "config", description = "Configuration endpoints"),
    )
)]
pub struct ApiDoc;
```

- [ ] **Step 3.3: 在 `services/config-service/src/lib.rs` 顶部加** `pub mod openapi;`

- [ ] **Step 3.4: 升级 `services/config-service/src/routes/mod.rs`** — 补 `/health` `/metrics`：

```rust
pub mod namespace;
pub mod config;
pub mod watch;

use axum::{Router, routing::get};
use std::sync::Arc;
use crate::services::ConfigService;
use prometheus::{Encoder, TextEncoder};

pub type AppState = Arc<ConfigService>;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/config", namespace::routes(state.clone()))
        .nest("/config", config::routes(state.clone()))
        .nest("/config", watch::routes(state.clone()))
        .route("/health", get(health))
        .route("/metrics", get(metrics))
}

async fn health() -> &'static str {
    "OK"
}

async fn metrics() -> axum::response::Html<String> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    axum::response::Html(String::from_utf8(buffer).unwrap())
}

pub fn create_state(pool: digua_common_db::DbPool) -> AppState {
    Arc::new(ConfigService::new(pool))
}
```

- [ ] **Step 3.5: 升级 `services/config-service/src/main.rs`** — 接 SWAGGER_ENABLED：

```rust
use config_service::{config::AppConfig, routes::{self, create_state}};
use digua_common_db::init_pool;
use digua_common_log::init_json;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    init_json("info");

    let config = AppConfig::from_env();
    tracing::info!("Starting config-service on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    let state = create_state(pool);

    let mut app = routes::routes(state);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false);
    if swagger_enabled {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        app = app.merge(
            utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", config_service::openapi::ApiDoc::openapi()),
        );
        tracing::info!("swagger ui enabled at /swagger-ui");
    }

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");

    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    tracing::info!("config-service listening on {}", addr);

    axum::serve(listener, app).await.expect("Server error");
}
```

- [ ] **Step 3.6: 重写 `services/config-service/AGENTS.md`** — 用统一模板（端点域写 config 自己的路径）：

```markdown
# config-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | 数据库连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /config/namespaces | GET / POST | 命名空间管理 |
| /config/items | GET / POST / PUT / DELETE | 配置项 CRUD |
| /config/watch | GET (SSE) | 配置变更订阅 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 豁免

（无）

## AI 检查时机

修改 services/config-service/ 下文件时自动检查
```

- [ ] **Step 3.7: 编译与跑**

```bash
node scripts/run-tests-docker.js config-service
```

Expected: PASS

- [ ] **Step 3.8: 跑规范脚本**

```bash
node scripts/check-service-standard.js
```

Expected: audit + config 两服务 8 项全 ✓，其它仍 ⚠️（脚本 exit 1）

- [ ] **Step 3.9: 提交**

```bash
git add services/config-service
git commit -m "feat(config-service): add /health /metrics, SWAGGER_ENABLED switch, align AGENTS.md"
```

---

## Task 4: file-service 规范收口

**Files:**
- Modify: `services/file-service/Cargo.toml`
- Modify: `services/file-service/src/lib.rs`
- Modify: `services/file-service/src/main.rs`
- Create: `services/file-service/src/openapi.rs`
- Modify: `services/file-service/AGENTS.md`

**Interfaces:**
- Consumes: Task 1 脚本规则
- Produces: file-service 通过 8 项检查

- [ ] **Step 4.1: 加 Cargo 依赖** — 在 `[dependencies]` 后插入 utoipa 两行（保持其它行原样）：

```toml
prometheus = "0.13"
utoipa = { version = "5", features = ["axum_extras", "chrono"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
```

最终 `services/file-service/Cargo.toml` 的 `[dependencies]` 段：

```toml
[dependencies]
axum.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
sqlx.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
bytes.workspace = true
futures.workspace = true
prometheus = "0.13"
utoipa = { version = "5", features = ["axum_extras", "chrono"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
tokio-util = { version = "0.7", features = ["io"] }
async-trait = "0.1"
digua_common-error = { path = "../../libs/digua_common-error" }
digua_common-db = { path = "../../libs/digua_common-db", features = ["mysql", "sqlite"] }
digua_common-log = { path = "../../libs/digua_common-log" }
digua_common-cache = { path = "../../libs/digua_common-cache" }
```

- [ ] **Step 4.2: 新建 `services/file-service/src/openapi.rs`**

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "file-service", description = "File upload, download and presign"),
    tags(
        (name = "file", description = "File endpoints"),
    )
)]
pub struct ApiDoc;
```

- [ ] **Step 4.3: 在 `services/file-service/src/lib.rs` 顶部加** `pub mod openapi;`

- [ ] **Step 4.4: 升级 `services/file-service/src/main.rs`** — 接 SWAGGER_ENABLED：

```rust
use file_service::{config::AppConfig, routes::{self, create_state}, storage::LocalStorage};
use digua_common_db::init_pool;
use digua_common_log::init_json;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    init_json("info");

    let config = AppConfig::from_env();
    tracing::info!("Starting file-service on {}", config.server_addr);

    std::fs::create_dir_all(&config.storage_path).expect("Failed to create storage directory");

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    let storage = Arc::new(LocalStorage::new(&config.storage_path));
    let base_url = format!("http://localhost:8082/file/");

    let state = create_state(pool, storage, base_url);

    let mut app = routes::routes(state);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false);
    if swagger_enabled {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", file_service::openapi::ApiDoc::openapi()),
        );
        tracing::info!("swagger ui enabled at /swagger-ui");
    }

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");

    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");
    tracing::info!("file-service listening on {}", addr);

    axum::serve(listener, app).await.expect("Server error");
}
```

- [ ] **Step 4.5: 重写 `services/file-service/AGENTS.md`**

```markdown
# file-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | 数据库连接 |
| STORAGE_PATH | ./data/files | 本地存储目录 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /file/upload | POST | 文件上传 |
| /file/download/:id | GET | 文件下载 |
| /file/presign | POST | 预签名 URL |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 豁免

（无）

## AI 检查时机

修改 services/file-service/ 下文件时自动检查
```

- [ ] **Step 4.6: 编译与跑**

```bash
node scripts/run-tests-docker.js file-service
```

Expected: PASS

- [ ] **Step 4.7: 提交**

```bash
git add services/file-service
git commit -m "feat(file-service): SWAGGER_ENABLED switch, align AGENTS.md"
```

---

## Task 5: prue_backend 规范收口

**Files:**
- Modify: `services/prue_backend/Cargo.toml`
- Modify: `services/prue_backend/src/lib.rs`
- Modify: `services/prue_backend/src/main.rs`
- Create: `services/prue_backend/src/openapi.rs`
- Modify: `services/prue_backend/AGENTS.md`

**Interfaces:**
- Consumes: Task 1 脚本规则
- Produces: prue_backend 通过 8 项检查

- [ ] **Step 5.1: 加 Cargo 依赖** — 在 `services/prue_backend/Cargo.toml` `[dependencies]` 段末尾追加：

```toml
utoipa = { version = "5", features = ["axum_extras", "chrono"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
```

- [ ] **Step 5.2: 新建 `services/prue_backend/src/openapi.rs`**

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "prue_backend", description = "pure-admin menu config backend"),
    tags(
        (name = "menu-configs", description = "Menu config versions"),
    )
)]
pub struct ApiDoc;
```

- [ ] **Step 5.3: 在 `services/prue_backend/src/lib.rs` 顶部加** `pub mod openapi;`

- [ ] **Step 5.4: 升级 `services/prue_backend/src/main.rs`** — 接 SWAGGER_ENABLED：

```rust
use digua_common_db::init_pool;
use digua_common_log::init_json;
use prue_backend::{config::AppConfig, db, routes, state::AppState};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let config = AppConfig::from_env();
    init_json(&config.log_level);

    tracing::info!("Starting prue_backend on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    if let Err(e) = db::ensure_schema(&pool).await {
        tracing::warn!(error = %e, "prue_backend schema initialization skipped");
    }

    let state = AppState::new(pool);

    let mut app = routes::routes(state);

    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(false);
    if swagger_enabled {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", prue_backend::openapi::ApiDoc::openapi()),
        );
        tracing::info!("swagger ui enabled at /swagger-ui");
    }

    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
```

- [ ] **Step 5.5: 重写 `services/prue_backend/AGENTS.md`** — 保留外部文档段 + 服务边界段，必需组件 / 环境变量 / 端点域用统一模板：

```markdown
# prue_backend 服务规范

## 外部文档

- pure-admin 文档：https://pure-admin.cn/pages/introduction/
- pure-admin 后端参考：https://github.com/pure-admin/pure-admin-backend

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | sqlite::memory: | 数据库连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |

## 服务边界

- 只管理 pure-admin 菜单配置和动态路由。
- 菜单配置使用版本化 JSON 持久化。
- 用户和角色仅保存为字符串数组。
- 不关联 iam-service。
- 不管理用户、角色、权限实体。

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /get-async-routes | GET | 获取 active 菜单版本的 pure-admin 动态路由 |
| /pure/menu-configs | GET | 查询菜单配置版本列表 |
| /pure/menu-configs | POST | 新建草稿菜单配置 |
| /pure/menu-configs/{id} | GET | 查看菜单配置版本详情 |
| /pure/menu-configs/{id} | PUT | 更新草稿菜单配置 |
| /pure/menu-configs/{id}/publish | POST | 发布菜单配置为 active |
| /pure/menu-configs/{id}/clone | POST | 克隆历史版本为草稿 |
| /pure/menu-configs/{id} | DELETE | 删除未发布菜单配置 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 豁免

（无）

## 数据库方言

支持 MySQL 和 SQLite。

## AI 检查时机

修改 services/prue_backend/ 下文件时自动检查
```

- [ ] **Step 5.6: 编译与跑**

```bash
node scripts/run-tests-docker.js prue_backend
```

Expected: PASS

- [ ] **Step 5.7: 提交**

```bash
git add services/prue_backend
git commit -m "feat(prue_backend): SWAGGER_ENABLED switch, align AGENTS.md"
```

---

## Task 6: notification-service / token-service / iam-service AGENTS.md 对齐

**Files:**
- Modify: `services/notification-service/AGENTS.md`
- Modify: `services/token-service/AGENTS.md`
- Modify: `services/iam-service/AGENTS.md`

**Interfaces:**
- Consumes: Task 1 脚本规则
- Produces: 这 3 个服务全部 8 项 ✓

> 三个服务的源码已满足 8 项；只需把 AGENTS.md 升级到统一模板。

- [ ] **Step 6.1: 重写 `services/notification-service/AGENTS.md`**

```markdown
# notification-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | MySQL 连接 |
| REDIS_URL | 服务自定义 | Redis 连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /notification/announcements | POST | 新建公告 |
| /notification/announcements/:id/publish | POST | 发布 |
| /notification/announcements/:id/revoke | POST | 撤销 |
| /notification/announcements/:id | GET | 详情 |
| /notification/mine | GET | 个人通知列表 |
| /notification/mine/:id/read | POST | 标记已读 |
| /notification/mine/unread-count | GET | 未读数 |
| /notification/ws | GET (WS) | WebSocket 推送 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 豁免

（无）

## AI 检查时机

修改 services/notification-service/ 下文件时自动检查
```

- [ ] **Step 6.2: 重写 `services/token-service/AGENTS.md`**

```markdown
# token-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | MySQL 连接 |
| REDIS_URL | 服务自定义 | Redis 连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /token/issue | POST | 签发 token |
| /token/refresh | POST | 刷新 token |
| /token/revoke | POST | 撤销 token |
| /token/jwks | GET | JWKS 公钥 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 豁免

（无）

## AI 检查时机

修改 services/token-service/ 下文件时自动检查
```

- [ ] **Step 6.3: 重写 `services/iam-service/AGENTS.md`** — 保留完整端点表（auth / users / roles / permissions / org-units）+ 状态说明，必需组件 / 环境变量段统一：

```markdown
# iam-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | 数据库连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |

## 端点域

### 认证 /iam/auth/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/auth/login | POST | 用户登录 |
| /iam/auth/refresh | POST | 刷新 Token |
| /iam/auth/logout | POST | 用户登出 |
| /iam/auth/password | POST | 修改密码 |
| /iam/auth/sessions | GET | 获取会话列表 |
| /iam/auth/sessions/:id | DELETE | 撤销会话 |

### 用户 /iam/users/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/users | GET | 查询用户列表 |
| /iam/users | POST | 创建用户 |
| /iam/users/:id | GET | 获取用户详情 |
| /iam/users/:id | PUT | 更新用户 |
| /iam/users/:id | DELETE | 删除用户 |

### 角色 /iam/roles/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/roles | GET | 查询角色列表 |
| /iam/roles | POST | 创建角色 |
| /iam/roles/:id | GET | 获取角色详情 |
| /iam/roles/:id | PUT | 更新角色 |
| /iam/roles/:id | DELETE | 删除角色 |
| /iam/roles/:id/permissions | PUT | 绑定权限 |

### 权限 /iam/permissions/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/permissions | GET | 查询权限列表 |
| /iam/permissions | POST | 创建权限 |
| /iam/permissions/:id | GET | 获取权限详情 |
| /iam/permissions/:id | PUT | 更新权限 |
| /iam/permissions/:id | DELETE | 删除权限 |

### 组织单元 /iam/org-units/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/org-units | GET | 查询组织列表（树形） |
| /iam/org-units | POST | 创建组织 |
| /iam/org-units/:id | GET | 获取组织详情 |
| /iam/org-units/:id | PUT | 更新组织 |
| /iam/org-units/:id | DELETE | 删除组织 |

### 系统

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 数据库方言

支持 MySQL 和 SQLite。

## 豁免

（无）

## 状态说明

本服务整合了 token-service、credential-login-service、user-service 的功能。

## AI 检查时机

修改 services/iam-service/ 下文件时自动检查
```

- [ ] **Step 6.4: 跑规范脚本**

```bash
node scripts/check-service-standard.js
```

Expected: 这 3 服务 8 项全 ✓；剩下只有 api-gateway 未对齐（脚本仍 exit 1）

- [ ] **Step 6.5: 提交**

```bash
git add services/notification-service/AGENTS.md services/token-service/AGENTS.md services/iam-service/AGENTS.md
git commit -m "docs(agents): align notification/token/iam AGENTS.md to unified template"
```

---

## Task 7: api-gateway 规范收口（含文件名重命名 + 豁免）

**Files:**
- Rename: `services/api-gateway/agents.md` → `services/api-gateway/AGENTS.md`
- Modify (the new file): `services/api-gateway/AGENTS.md`

**Interfaces:**
- Consumes: Task 1 脚本规则（豁免段识别）
- Produces: api-gateway 通过检查（含 2 项豁免：migrations / Swagger）

> api-gateway 无业务 DB → 豁免 `migrations/ 目录`；Swagger 聚合视图留作 P2.1 → 豁免 `Swagger 配置`。本任务只统一文件名与模板。

- [ ] **Step 7.1: 重命名**

```bash
git mv services/api-gateway/agents.md services/api-gateway/AGENTS.md
```

> 备注：Linux 文件系统大小写敏感；`git mv` 会显式记录重命名。

- [ ] **Step 7.2: 重写 `services/api-gateway/AGENTS.md`**

```markdown
# api-gateway 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [ ] migrations/ 目录（豁免，见 ## 豁免）
- [ ] Swagger 配置（豁免，聚合视图留作后续阶段）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 3000 | 监听端口（对外网关固定 3000）|
| SERVER_ADDR | 0.0.0.0:3000 | 完整监听地址 |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 预留（聚合 UI 待 P2.1）|

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /__admin/reload | POST | 热加载路由配置 |
| /api/v1/* | * | 业务路由代理（按 routes/*.router.toml 转发）|

## 豁免

- migrations/ 目录
- Swagger 配置（SWAGGER_ENABLED 开关）

## AI 检查时机

修改 services/api-gateway/ 下文件时自动检查
```

- [ ] **Step 7.3: 跑规范脚本**

```bash
node scripts/check-service-standard.js
```

Expected: 8 服务全 ✓（含 api-gateway 的 2 项豁免输出 `◯ 豁免: ...`），脚本 exit 0

- [ ] **Step 7.4: 提交**

```bash
git add services/api-gateway
git commit -m "chore(api-gateway): rename agents.md → AGENTS.md, declare exemptions"
```

---

## Task 8: `contracts:check` 子命令 + 历史日志清理

**Files:**
- Modify: `scripts/contracts-gen.js`
- Modify: `.gitignore`
- Delete: `build-gw.log`, `build-iam.log`

**Interfaces:**
- Produces: `node scripts/contracts-gen.js --check` exit 0 / 1，CI 使用

- [ ] **Step 8.1: 升级 `scripts/contracts-gen.js`** — 替换整个文件：

```javascript
#!/usr/bin/env node
/**
 * 契约代码生成入口
 * - 默认：调用 contracts/codegen/*.js 生成各语言代码
 * - --check：生成后用 git diff 检查产物是否漂移，漂移则 exit 1
 */

const path = require('path');
const { execSync } = require('child_process');

const codegenDir = path.resolve(__dirname, '../contracts/codegen');
const repoRoot = path.resolve(__dirname, '..');
const isCheck = process.argv.includes('--check');

console.log('=== Contracts Code Generation ===');
console.log('');

console.log('1. Generating Rust error codes...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust error codes');
  process.exit(1);
}

console.log('');
console.log('2. Generating Rust permission codes...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust-perms.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust permission codes');
  process.exit(1);
}

console.log('');
console.log('3. Generating TypeScript contracts...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-ts.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.log('TypeScript generation skipped (frontend not ready)');
}

console.log('');
console.log('=== Done ===');

if (isCheck) {
  console.log('');
  console.log('4. Checking for drift...');
  try {
    execSync('git diff --exit-code -- libs/digua_common-error libs/digua_authz frontend/src/contracts', {
      cwd: repoRoot,
      stdio: 'inherit',
    });
    console.log('✅ Contracts in sync');
  } catch (e) {
    console.error('');
    console.error('❌ Contract drift detected. Run `pnpm run contracts:gen` and commit the result.');
    process.exit(1);
  }
}
```

- [ ] **Step 8.2: 跑两次** — 验证幂等：

```bash
node scripts/contracts-gen.js --check
```

Expected: 输出 `✅ Contracts in sync`, exit 0
（若 exit 1，说明仓库已有契约漂移；本步骤要先把漂移 commit 干净再继续，并将该 commit 与本任务一起提交）

- [ ] **Step 8.3: 升级 `.gitignore`** — 末尾追加：

```
/build-*.log
```

- [ ] **Step 8.4: 删除根目录历史日志**

```bash
git rm build-gw.log build-iam.log
```

- [ ] **Step 8.5: 提交**

```bash
git add scripts/contracts-gen.js .gitignore
git commit -m "feat(contracts-gen): add --check subcommand; chore: ignore build-*.log"
```

---

## Task 9: `.gitlab-ci.yml` 串联三个闸门

**Files:**
- Modify: `.gitlab-ci.yml`

**Interfaces:**
- Consumes: Task 1 `check:service-standard`、Task 8 `contracts:check`
- Produces: CI test 阶段强制 3 个闸门

- [ ] **Step 9.1: 在 `service_standard_check` job 之后插入两个 job** — 用 Edit 把：

```yaml
service_standard_check:
  stage: test
  tags: [docker]
  image: node:20
  script:
    - node scripts/check-service-standard.js
```

替换为：

```yaml
service_standard_check:
  stage: test
  tags: [docker]
  image: node:20
  script:
    - node scripts/check-service-standard.js

spec_check:
  stage: test
  tags: [docker]
  image: node:20
  script:
    - node scripts/check-spec.js

contracts_check:
  stage: test
  tags: [docker]
  image: node:20
  script:
    - node scripts/contracts-gen.js --check
```

- [ ] **Step 9.2: 本地等价验证**

```bash
pnpm run check:service-standard
pnpm run check:spec
pnpm run contracts:check
```

Expected: 全部 exit 0

- [ ] **Step 9.3: 提交**

```bash
git add .gitlab-ci.yml
git commit -m "ci: enforce check:spec and contracts:check as test gates"
```

---

## Task 10: ADR-002 + 终验

**Files:**
- Create: `docs-site/decisions/ADR-002-service-standard-baseline.md`

**Interfaces:**
- Produces: 文档化的硬基线，新服务复用模板必须遵守

- [ ] **Step 10.1: 写 ADR**

```markdown
# ADR-002: Service Standard Baseline

> 日期：2026-06-25
> 状态：已采纳
> 关联：[P1.3 设计](../superpowers/specs/2026-06-25-service-standard-sweep-design.md) / [P1.3 实施](../superpowers/plans/2026-06-25-service-standard-sweep-plan.md)

## 上下文

随服务数量从 2 长到 8，各服务 AGENTS.md、端点接入、环境变量命名出现漂移。`scripts/check-service-standard.js` 早期规则只识别 6 项，且把"健康检查端点"判定为不存在的 `src/health.rs`，无法守护规范。

## 决议

1. **必需组件 8 项**（每个服务必须满足，豁免必须显式声明）：
   - Dockerfile / AGENTS.md / README.md / tests/ 目录
   - `/health` 端点 / `/metrics` 端点
   - `migrations/` 目录（无 DB 服务豁免）
   - Swagger 配置（`SWAGGER_ENABLED` 开关 + `utoipa-swagger-ui` 依赖）
2. **统一环境变量名**：`SERVER_HOST` / `SERVER_PORT` / `SERVER_ADDR` / `DATABASE_URL` / `RUST_LOG` / `SWAGGER_ENABLED` / `CONSUL_ADDR`
3. **豁免规则**：在 `AGENTS.md` 的 `## 豁免` 段每行一项写出，被检查脚本识别并跳过
4. **CI 强制**：`check:service-standard` / `check:spec` / `contracts:check` 三项 test 阶段闸门，任一红则阻断 build/deploy
5. **新服务**：必须从 `services/agents-template.md` 复制 AGENTS.md

## 后果

- 新服务上线必须先满足 8 项；CI 自动守护，避免人工 review 遗漏
- `check-service-standard.js` 与模板是绑定关系：模板改了脚本规则必须同步改
- Swagger 默认 `SWAGGER_ENABLED=false`；生产开启需在部署清单中显式声明
- 历史日志（build-*.log）禁止提交（`.gitignore` 拦截）
```

- [ ] **Step 10.2: 终验 Demo** — 全部命令 exit 0：

```bash
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
git diff --exit-code
node scripts/run-tests-docker.js
```

注：`scripts/run-tests-docker.js` 跑全部服务串行，约 5–10 分钟，全 PASS 才继续。

- [ ] **Step 10.3: 端到端 e2e（可选 / 推荐）**

```bash
docker compose -f infrastructure/docker-compose.yml up -d
pnpm run test:e2e
docker compose -f infrastructure/docker-compose.yml down
```

Expected: test:e2e 全套 PASS

- [ ] **Step 10.4: 提交**

```bash
git add docs-site/decisions/ADR-002-service-standard-baseline.md
git commit -m "docs(adr): ADR-002 service standard baseline"
```

---

## Self-Review

按 spec 第 12 节验收清单逐项对照：

- [x] 8 个服务 `AGENTS.md` 全部对齐（Tasks 2 / 3 / 4 / 5 / 6 / 7）
- [x] api-gateway 文件名从 `agents.md` 改为 `AGENTS.md`（Task 7.1）
- [x] audit-service 补 `/metrics` + SwaggerUi 开关（Task 2）
- [x] config-service 补 `/health` + `/metrics` + SwaggerUi 开关（Task 3）
- [x] file-service 补 SwaggerUi 开关（Task 4）
- [x] prue_backend 补 SwaggerUi 开关（Task 5）
- [x] `services/agents-template.md` 升级为完整模板（Task 1.1）
- [x] `scripts/check-service-standard.js` 升级为 8 项规则 + 豁免段 + exit code（Task 1.2）
- [x] `scripts/contracts-gen.js --check` 子命令落地（Task 8.1）
- [x] `.gitlab-ci.yml` test 阶段串 3 闸门（Task 9）
- [x] `.gitignore` 加 `build-*.log`，删除根目录两个历史日志（Task 8.3–8.4）
- [x] `docs-site/decisions/ADR-002-service-standard-baseline.md` 提交（Task 10.1）
- [x] 所有验收 Demo 命令 exit 0（Task 10.2–10.3）

Placeholder 扫描：每个 Task 含完整代码块与命令，无 TBD/TODO/"similar to" 引用。
Type/方法名一致性：`audit_service::openapi::ApiDoc`、`config_service::openapi::ApiDoc`、`file_service::openapi::ApiDoc`、`prue_backend::openapi::ApiDoc` 与各 `src/openapi.rs` 定义匹配；`SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ...)` 在 4 个服务中字符串一致。
