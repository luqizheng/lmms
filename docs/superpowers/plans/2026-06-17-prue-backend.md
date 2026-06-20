# prue_backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增 `prue_backend` Rust 服务，使用版本化 JSON 持久化 pure-admin 菜单配置，并从 `iam-service` 移除菜单职责。

**Architecture:** `prue_backend` 是独立 BFF 服务，不关联 `iam-service`。服务将完整 pure-admin 路由树保存为 JSON 版本快照，`/get-async-routes` 只读取 active 版本。

**Tech Stack:** Rust 2021、Axum 0.8、SQLx 0.8、MySQL/SQLite、serde_json、api-gateway TOML、Docker Compose。

---

## 文件结构

### 新增

- `services/prue_backend/Cargo.toml`：服务 crate 配置。
- `services/prue_backend/Dockerfile`：Linux/musl 容器构建。
- `services/prue_backend/README.md`：服务说明。
- `services/prue_backend/AGENTS.md`：服务规范和 pure-admin 链接。
- `services/prue_backend/src/lib.rs`：模块导出。
- `services/prue_backend/src/main.rs`：服务启动入口。
- `services/prue_backend/src/config.rs`：环境变量配置。
- `services/prue_backend/src/error.rs`：服务错误类型。
- `services/prue_backend/src/state.rs`：AppState。
- `services/prue_backend/src/dto/mod.rs`：DTO 导出。
- `services/prue_backend/src/dto/menu_config.rs`：请求和响应结构。
- `services/prue_backend/src/service/mod.rs`：服务导出。
- `services/prue_backend/src/service/menu_config_svc.rs`：菜单版本业务逻辑。
- `services/prue_backend/src/routes/mod.rs`：路由组装。
- `services/prue_backend/src/routes/menu_configs.rs`：菜单配置接口。
- `services/prue_backend/migrations/mysql/V001__init.sql`：MySQL 表结构。
- `services/prue_backend/migrations/sqlite/V001__init.sql`：SQLite 表结构。
- `services/prue_backend/tests/menu_config_model_test.rs`：DTO 和校验单测。
- `docker/config/api-gateway/routes/prue_backend.router.toml`：网关路由。

### 修改

- `Cargo.toml`：加入 `services/prue_backend` workspace member。
- `infrastructure/docker-compose.yml`：新增 `prue_backend` 服务，端口 `6004:8080`。
- `services/iam-service/src/routes/mod.rs`：移除 menus 模块和路由合并。
- `services/iam-service/src/dto/mod.rs`：移除 menu DTO 导出。
- `services/iam-service/src/models/mod.rs`：移除 menu model 导出。
- `services/iam-service/src/service/mod.rs`：移除 menu service 导出。
- `services/iam-service/src/openapi.rs`：移除 menus OpenAPI 引用。
- `services/iam-service/README.md`：移除菜单管理描述。
- `services/iam-service/AGENTS.md`：移除菜单端点描述。
- `services/iam-service/tests/swagger_test.rs`：移除 `/iam/menus*` 断言。
- `docker/config/api-gateway/routes/iam.router.toml`：移除 `/iam/menus/**`。

### 删除

- `services/iam-service/src/routes/menus.rs`
- `services/iam-service/src/dto/menu.rs`
- `services/iam-service/src/models/menu.rs`
- `services/iam-service/src/service/menu_svc.rs`

---

## Task 1: 新服务骨架

**Files:**
- Modify: `Cargo.toml`
- Create: `services/prue_backend/Cargo.toml`
- Create: `services/prue_backend/src/lib.rs`
- Create: `services/prue_backend/src/config.rs`
- Create: `services/prue_backend/src/state.rs`
- Create: `services/prue_backend/src/main.rs`
- Create: `services/prue_backend/AGENTS.md`
- Create: `services/prue_backend/README.md`
- Create: `services/prue_backend/Dockerfile`

- [ ] **Step 1: 修改 workspace**

在根 `Cargo.toml` 的 `members` 中追加：

```toml
    "services/prue_backend",
```

放在其他 `services/*` 后面。

- [ ] **Step 2: 创建服务 Cargo.toml**

写入 `services/prue_backend/Cargo.toml`：

```toml
[package]
name = "prue_backend"
version = "0.1.0"
edition = "2021"
default-run = "prue_backend"

[dependencies]
common-db = { path = "../../libs/common-db", features = ["mysql", "sqlite"] }
common-log = { path = "../../libs/common-log" }

axum.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls-aws-lc-rs", "mysql", "sqlite", "chrono", "json"] }
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
nanoid = "0.4"
prometheus = "0.13"

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }

[[bin]]
name = "prue_backend"
path = "src/main.rs"
```

- [ ] **Step 3: 创建 lib.rs**

写入 `services/prue_backend/src/lib.rs`：

```rust
pub mod config;
pub mod dto;
pub mod error;
pub mod routes;
pub mod service;
pub mod state;
```

- [ ] **Step 4: 创建 config.rs**

写入 `services/prue_backend/src/config.rs`：

```rust
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
    pub log_level: String,
}

fn default_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

fn default_server_addr() -> String {
    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
    std::env::var("SERVER_ADDR").unwrap_or_else(|_| format!("{host}:{port}"))
}

fn default_log_level() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: default_database_url(),
            server_addr: default_server_addr(),
            log_level: default_log_level(),
        }
    }
}
```

- [ ] **Step 5: 创建 state.rs**

写入 `services/prue_backend/src/state.rs`：

```rust
use common_db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
}

impl AppState {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}
```

- [ ] **Step 6: 创建 main.rs**

写入 `services/prue_backend/src/main.rs`：

```rust
use common_db::init_pool;
use common_log::init_json;
use prue_backend::{config::AppConfig, routes, state::AppState};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let config = AppConfig::from_env();
    init_json(&config.log_level);

    tracing::info!("Starting prue_backend on {}", config.server_addr);

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database pool");

    let state = AppState::new(pool);
    let addr: SocketAddr = config.server_addr.parse().expect("Invalid server address");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");

    axum::serve(listener, routes::routes(state))
        .await
        .expect("Server error");
}
```

- [ ] **Step 7: 创建 AGENTS.md**

写入 `services/prue_backend/AGENTS.md`：

```markdown
# prue_backend 服务规范

## 外部文档

- pure-admin 文档：https://pure-admin.cn/pages/introduction/
- pure-admin 后端参考：https://github.com/pure-admin/pure-admin-backend

## 必需组件

- [x] Dockerfile
- [x] 健康检查端点（/health、/metrics）
- [x] 数据库迁移文件（migrations/）
- [x] README.md
- [x] tests/ 目录

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | sqlite::memory: | 数据库连接 |
| SERVER_HOST | 0.0.0.0 | 服务监听主机 |
| SERVER_PORT | 8080 | 服务监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| RUST_LOG | info | 日志级别 |

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

## 数据库方言

支持 MySQL 和 SQLite。
```

- [ ] **Step 8: 创建 README.md**

写入 `services/prue_backend/README.md`：

````markdown
# prue_backend

pure-admin 后台适配服务。

## 功能

- 管理菜单配置版本
- JSON 持久化 pure-admin 路由树
- 发布 active 菜单版本
- 提供 `/get-async-routes`

## 边界

用户和角色只保存字符串数组。

服务不关联 iam-service。

## 运行

```powershell
$env:DATABASE_URL="sqlite::memory:"
$env:SERVER_ADDR="0.0.0.0:8080"
prue_backend
```
````

- [ ] **Step 9: 创建 Dockerfile**

写入 `services/prue_backend/Dockerfile`：

```dockerfile
FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder

USER root
WORKDIR /app

RUN apt-get update && apt-get install -y libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

COPY . .

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release --target x86_64-unknown-linux-musl --bin prue_backend && \
    cp /app/target/x86_64-unknown-linux-musl/release/prue_backend /app/prue_backend

FROM alpine:3.21

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/prue_backend /usr/local/bin/prue_backend

EXPOSE 8080

CMD ["prue_backend"]
```

---

## Task 2: DTO、错误和 JSON 校验

**Files:**
- Create: `services/prue_backend/src/error.rs`
- Create: `services/prue_backend/src/dto/mod.rs`
- Create: `services/prue_backend/src/dto/menu_config.rs`
- Create: `services/prue_backend/tests/menu_config_model_test.rs`

- [ ] **Step 1: 创建 error.rs**

写入 `services/prue_backend/src/error.rs`：

```rust
use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Serialize)]
struct ErrorBody {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, 404, "not found".to_string()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, 400, msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, 409, msg),
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, 500, "internal error".to_string()),
        };

        (status, Json(ErrorBody { code, message, data: None })).into_response()
    }
}
```

- [ ] **Step 2: 创建 dto/mod.rs**

写入 `services/prue_backend/src/dto/mod.rs`：

```rust
pub mod menu_config;

pub use menu_config::*;
```

- [ ] **Step 3: 创建 dto/menu_config.rs**

写入 `services/prue_backend/src/dto/menu_config.rs`：

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            message: "ok".to_string(),
            data,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateMenuConfigRequest {
    pub name: String,
    pub description: Option<String>,
    pub routes_json: Value,
    pub created_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateMenuConfigRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub routes_json: Option<Value>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MenuConfigSummary {
    pub id: String,
    pub version: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub is_active: bool,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MenuConfigDetail {
    pub id: String,
    pub version: i64,
    pub name: String,
    pub description: Option<String>,
    pub routes_json: Value,
    pub status: String,
    pub is_active: bool,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct MenuConfigListResponse {
    pub items: Vec<MenuConfigSummary>,
    pub total: u64,
}

pub fn validate_routes_json(value: &Value) -> Result<(), String> {
    let routes = value.as_array().ok_or("routes_json must be an array")?;
    let mut paths = HashSet::new();
    let mut names = HashSet::new();

    for route in routes {
        validate_route(route, &mut paths, &mut names)?;
    }

    Ok(())
}

fn validate_route(
    route: &Value,
    paths: &mut HashSet<String>,
    names: &mut HashSet<String>,
) -> Result<(), String> {
    let obj = route.as_object().ok_or("route must be an object")?;

    let path = obj
        .get("path")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or("route.path is required")?;
    if !paths.insert(path.to_string()) {
        return Err(format!("duplicate route.path: {path}"));
    }

    if let Some(name) = obj.get("name").and_then(Value::as_str).filter(|v| !v.trim().is_empty()) {
        if !names.insert(name.to_string()) {
            return Err(format!("duplicate route.name: {name}"));
        }
    }

    if let Some(meta) = obj.get("meta") {
        let meta_obj = meta.as_object().ok_or("route.meta must be an object")?;
        validate_string_array(meta_obj.get("roles"), "route.meta.roles")?;
        validate_string_array(meta_obj.get("auths"), "route.meta.auths")?;
    }

    if let Some(children) = obj.get("children") {
        let child_routes = children.as_array().ok_or("route.children must be an array")?;
        for child in child_routes {
            validate_route(child, paths, names)?;
        }
    }

    Ok(())
}

fn validate_string_array(value: Option<&Value>, field: &str) -> Result<(), String> {
    if let Some(value) = value {
        let arr = value.as_array().ok_or_else(|| format!("{field} must be an array"))?;
        if arr.iter().any(|item| item.as_str().is_none()) {
            return Err(format!("{field} must contain strings only"));
        }
    }
    Ok(())
}
```

- [ ] **Step 4: 创建 DTO 测试**

写入 `services/prue_backend/tests/menu_config_model_test.rs`：

```rust
use prue_backend::dto::{validate_routes_json, ApiResponse, CreateMenuConfigRequest};
use serde_json::json;

#[test]
fn api_response_uses_pure_admin_shape() {
    let response = ApiResponse::ok(json!([]));
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(value["code"], 0);
    assert_eq!(value["message"], "ok");
    assert!(value.get("data").is_some());
}

#[test]
fn create_menu_config_serializes_roles_and_auths_as_strings() {
    let req = CreateMenuConfigRequest {
        name: "main".to_string(),
        description: Some("default menu".to_string()),
        created_by: Some("admin".to_string()),
        routes_json: json!([
            {
                "path": "/system",
                "name": "System",
                "meta": {
                    "roles": ["admin"],
                    "auths": ["menu:list"]
                }
            }
        ]),
    };

    let value = serde_json::to_value(req).unwrap();
    assert_eq!(value["routes_json"][0]["meta"]["roles"][0], "admin");
    assert_eq!(value["routes_json"][0]["meta"]["auths"][0], "menu:list");
}

#[test]
fn validate_routes_json_accepts_nested_routes() {
    let routes = json!([
        {
            "path": "/system",
            "name": "System",
            "meta": { "roles": ["admin"] },
            "children": [
                {
                    "path": "/system/menu/index",
                    "name": "SystemMenu",
                    "component": "system/menu/index",
                    "meta": { "auths": ["menu:list", "menu:create"] }
                }
            ]
        }
    ]);

    assert!(validate_routes_json(&routes).is_ok());
}

#[test]
fn validate_routes_json_rejects_duplicate_path() {
    let routes = json!([
        { "path": "/system", "name": "System" },
        { "path": "/system", "name": "SystemCopy" }
    ]);

    let err = validate_routes_json(&routes).unwrap_err();
    assert!(err.contains("duplicate route.path"));
}

#[test]
fn validate_routes_json_rejects_non_string_roles() {
    let routes = json!([
        { "path": "/system", "name": "System", "meta": { "roles": [1] } }
    ]);

    let err = validate_routes_json(&routes).unwrap_err();
    assert!(err.contains("route.meta.roles"));
}
```

- [ ] **Step 5: 在真实 PowerShell 运行测试**

仓库规则禁止在 IDE 终端直接跑 cargo。

在真实 PowerShell（vcvarsall x64）运行：

```powershell
cargo test -p prue_backend --test menu_config_model_test
```

期望：测试通过。

---

## Task 3: 数据库迁移和业务服务

**Files:**
- Create: `services/prue_backend/migrations/mysql/V001__init.sql`
- Create: `services/prue_backend/migrations/sqlite/V001__init.sql`
- Create: `services/prue_backend/src/service/mod.rs`
- Create: `services/prue_backend/src/service/menu_config_svc.rs`

- [ ] **Step 1: 创建 MySQL 迁移**

写入 `services/prue_backend/migrations/mysql/V001__init.sql`：

```sql
CREATE TABLE IF NOT EXISTS menu_configs (
    id VARCHAR(64) PRIMARY KEY,
    version BIGINT NOT NULL,
    name VARCHAR(128) NOT NULL,
    description TEXT NULL,
    routes_json JSON NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'draft',
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_by VARCHAR(128) NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    published_at TIMESTAMP NULL,
    deleted_at TIMESTAMP NULL,
    UNIQUE KEY uk_menu_configs_version (version),
    INDEX idx_menu_configs_active (is_active),
    INDEX idx_menu_configs_status (status)
);
```

- [ ] **Step 2: 创建 SQLite 迁移**

写入 `services/prue_backend/migrations/sqlite/V001__init.sql`：

```sql
CREATE TABLE IF NOT EXISTS menu_configs (
    id TEXT PRIMARY KEY,
    version INTEGER NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NULL,
    routes_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    is_active INTEGER NOT NULL DEFAULT 0,
    created_by TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at TEXT NULL,
    deleted_at TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_menu_configs_active ON menu_configs(is_active);
CREATE INDEX IF NOT EXISTS idx_menu_configs_status ON menu_configs(status);
```

- [ ] **Step 3: 创建 service/mod.rs**

写入 `services/prue_backend/src/service/mod.rs`：

```rust
pub mod menu_config_svc;

pub use menu_config_svc::MenuConfigService;
```

- [ ] **Step 4: 创建 menu_config_svc.rs**

写入 `services/prue_backend/src/service/menu_config_svc.rs`：

```rust
use crate::{
    dto::{
        validate_routes_json, CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse,
        MenuConfigSummary, UpdateMenuConfigRequest,
    },
    error::{AppError, AppResult},
    state::AppState,
};
use chrono::{DateTime, Utc};
use nanoid::nanoid;
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct MenuConfigRow {
    id: String,
    version: i64,
    name: String,
    description: Option<String>,
    routes_json: String,
    status: String,
    is_active: bool,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
}

pub struct MenuConfigService {
    state: AppState,
}

impl MenuConfigService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn list(&self) -> AppResult<MenuConfigListResponse> {
        let rows: Vec<MenuConfigRow> = sqlx::query_as(
            "SELECT id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at FROM menu_configs WHERE deleted_at IS NULL ORDER BY version DESC"
        )
        .fetch_all(&self.state.db)
        .await?;

        let total = rows.len() as u64;
        let items = rows.into_iter().map(Self::to_summary).collect();

        Ok(MenuConfigListResponse { items, total })
    }

    pub async fn create(&self, req: CreateMenuConfigRequest) -> AppResult<MenuConfigDetail> {
        validate_routes_json(&req.routes_json).map_err(AppError::Validation)?;

        let id = nanoid!();
        let next_version = self.next_version().await?;
        let routes_json = serde_json::to_string(&req.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let row: MenuConfigRow = sqlx::query_as(
            "INSERT INTO menu_configs (id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'draft', false, ?, NOW(), NOW()) RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at"
        )
        .bind(&id)
        .bind(next_version)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&routes_json)
        .bind(&req.created_by)
        .fetch_one(&self.state.db)
        .await?;

        Self::to_detail(row)
    }

    pub async fn get(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let row = self.get_row(id).await?;
        Self::to_detail(row)
    }

    pub async fn update(&self, id: &str, req: UpdateMenuConfigRequest) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        if existing.status != "draft" {
            return Err(AppError::Conflict("only draft menu configs can be updated".to_string()));
        }

        let routes_value = match req.routes_json {
            Some(value) => {
                validate_routes_json(&value).map_err(AppError::Validation)?;
                value
            }
            None => serde_json::from_str(&existing.routes_json)
                .map_err(|e| AppError::Validation(e.to_string()))?,
        };

        let name = req.name.unwrap_or(existing.name);
        let description = req.description.or(existing.description);
        let routes_json = serde_json::to_string(&routes_value)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let row: MenuConfigRow = sqlx::query_as(
            "UPDATE menu_configs SET name = ?, description = ?, routes_json = ?, updated_at = NOW() WHERE id = ? AND deleted_at IS NULL RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at"
        )
        .bind(&name)
        .bind(&description)
        .bind(&routes_json)
        .bind(id)
        .fetch_one(&self.state.db)
        .await?;

        Self::to_detail(row)
    }

    pub async fn publish(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        let routes_value: Value = serde_json::from_str(&existing.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;
        validate_routes_json(&routes_value).map_err(AppError::Validation)?;

        let mut tx = self.state.db.begin().await?;

        sqlx::query("UPDATE menu_configs SET is_active = false WHERE deleted_at IS NULL")
            .execute(&mut *tx)
            .await?;

        let row: MenuConfigRow = sqlx::query_as(
            "UPDATE menu_configs SET status = 'published', is_active = true, published_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at"
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Self::to_detail(row)
    }

    pub async fn clone_config(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        let new_id = nanoid!();
        let next_version = self.next_version().await?;
        let cloned_name = format!("{} copy", existing.name);

        let row: MenuConfigRow = sqlx::query_as(
            "INSERT INTO menu_configs (id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'draft', false, ?, NOW(), NOW()) RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at"
        )
        .bind(&new_id)
        .bind(next_version)
        .bind(&cloned_name)
        .bind(&existing.description)
        .bind(&existing.routes_json)
        .bind(&existing.created_by)
        .fetch_one(&self.state.db)
        .await?;

        Self::to_detail(row)
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let existing = self.get_row(id).await?;
        if existing.status == "published" || existing.is_active {
            return Err(AppError::Conflict("published menu configs cannot be deleted".to_string()));
        }

        let result = sqlx::query("UPDATE menu_configs SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .execute(&self.state.db)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }

        Ok(())
    }

    pub async fn active_routes(&self) -> AppResult<Value> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT routes_json FROM menu_configs WHERE is_active = true AND deleted_at IS NULL ORDER BY version DESC LIMIT 1"
        )
        .fetch_optional(&self.state.db)
        .await?;

        match row {
            Some((routes_json,)) => serde_json::from_str(&routes_json)
                .map_err(|e| AppError::Validation(e.to_string())),
            None => Ok(Value::Array(vec![])),
        }
    }

    async fn next_version(&self) -> AppResult<i64> {
        let current: Option<(i64,)> = sqlx::query_as(
            "SELECT COALESCE(MAX(version), 0) FROM menu_configs WHERE deleted_at IS NULL"
        )
        .fetch_optional(&self.state.db)
        .await?;

        Ok(current.map(|v| v.0).unwrap_or(0) + 1)
    }

    async fn get_row(&self, id: &str) -> AppResult<MenuConfigRow> {
        let row: Option<MenuConfigRow> = sqlx::query_as(
            "SELECT id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at FROM menu_configs WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id)
        .fetch_optional(&self.state.db)
        .await?;

        row.ok_or(AppError::NotFound)
    }

    fn to_summary(row: MenuConfigRow) -> MenuConfigSummary {
        MenuConfigSummary {
            id: row.id,
            version: row.version,
            name: row.name,
            description: row.description,
            status: row.status,
            is_active: row.is_active,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            published_at: row.published_at,
        }
    }

    fn to_detail(row: MenuConfigRow) -> AppResult<MenuConfigDetail> {
        let routes_json = serde_json::from_str(&row.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        Ok(MenuConfigDetail {
            id: row.id,
            version: row.version,
            name: row.name,
            description: row.description,
            routes_json,
            status: row.status,
            is_active: row.is_active,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            published_at: row.published_at,
        })
    }
}
```

- [ ] **Step 5: 固化数据库执行方式**

把 `menu_config_svc.rs` 中所有数据库调用改成按 `DbPool` 分支执行。

先在文件顶部加入：

```rust
use common_db::DbPool;
```

把每个方法里的：

```rust
.fetch_all(&self.state.db)
.fetch_one(&self.state.db)
.fetch_optional(&self.state.db)
.execute(&self.state.db)
```

改为 match 结构。

`list()` 使用：

```rust
let rows: Vec<MenuConfigRow> = match &self.state.db {
    DbPool::Mysql(pool) => sqlx::query_as(sql).fetch_all(pool).await?,
    DbPool::Sqlite(pool) => sqlx::query_as(sql).fetch_all(pool).await?,
};
```

`get_row()` 使用：

```rust
let row: Option<MenuConfigRow> = match &self.state.db {
    DbPool::Mysql(pool) => sqlx::query_as(sql).bind(id).fetch_optional(pool).await?,
    DbPool::Sqlite(pool) => sqlx::query_as(sql).bind(id).fetch_optional(pool).await?,
};
```

`next_version()` 使用：

```rust
let current: (i64,) = match &self.state.db {
    DbPool::Mysql(pool) => sqlx::query_as(sql).fetch_one(pool).await?,
    DbPool::Sqlite(pool) => sqlx::query_as(sql).fetch_one(pool).await?,
};
```

写入类方法使用 MySQL/SQLite 两套 SQL。

MySQL 时间函数用 `NOW()`。

SQLite 时间函数用 `CURRENT_TIMESTAMP`。

每次 `INSERT ... RETURNING` 或 `UPDATE ... RETURNING` 都按方言分支执行。

事务发布也按 `DbPool::Mysql(pool)` 和 `DbPool::Sqlite(pool)` 分支各写一遍。

- [ ] **Step 6: 在真实 PowerShell 运行检查**

```powershell
cargo check -p prue_backend
```

期望：`prue_backend` 编译通过。

---

## Task 4: HTTP 路由

**Files:**
- Create: `services/prue_backend/src/routes/mod.rs`
- Create: `services/prue_backend/src/routes/menu_configs.rs`

- [ ] **Step 1: 创建 routes/mod.rs**

写入 `services/prue_backend/src/routes/mod.rs`：

```rust
pub mod menu_configs;

use axum::{routing::get, Router};
use prometheus::{Encoder, TextEncoder};
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .merge(menu_configs::routes(state))
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
```

- [ ] **Step 2: 创建 routes/menu_configs.rs**

写入 `services/prue_backend/src/routes/menu_configs.rs`：

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::Value;

use crate::{
    dto::{ApiResponse, CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse, UpdateMenuConfigRequest},
    error::AppResult,
    service::MenuConfigService,
    state::AppState,
};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/get-async-routes", get(get_async_routes))
        .route("/pure/menu-configs", get(list_menu_configs).post(create_menu_config))
        .route("/pure/menu-configs/{id}", get(get_menu_config).put(update_menu_config).delete(delete_menu_config))
        .route("/pure/menu-configs/{id}/publish", post(publish_menu_config))
        .route("/pure/menu-configs/{id}/clone", post(clone_menu_config))
        .with_state(state)
}

async fn get_async_routes(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<Value>>> {
    let service = MenuConfigService::new(state);
    let routes = service.active_routes().await?;
    Ok(Json(ApiResponse::ok(routes)))
}

async fn list_menu_configs(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<MenuConfigListResponse>>> {
    let service = MenuConfigService::new(state);
    let result = service.list().await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn create_menu_config(
    State(state): State<AppState>,
    Json(req): Json<CreateMenuConfigRequest>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.create(req).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn get_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.get(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn update_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMenuConfigRequest>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.update(&id, req).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn publish_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.publish(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn clone_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.clone_config(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn delete_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let service = MenuConfigService::new(state);
    service.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 3: 在真实 PowerShell 运行检查**

```powershell
cargo check -p prue_backend
```

期望：路由编译通过。

---

## Task 5: 网关和 Docker 编排

**Files:**
- Create: `docker/config/api-gateway/routes/prue_backend.router.toml`
- Modify: `infrastructure/docker-compose.yml`

- [ ] **Step 1: 创建网关路由**

写入 `docker/config/api-gateway/routes/prue_backend.router.toml`：

```toml
[[route]]
match = "/get-async-routes"
service = "prue_backend"
public = false

[[route]]
match = "/pure/**"
service = "prue_backend"
public = false

[[route]]
match = "/prue/health"
service = "prue_backend"
public = true
rewrite = { type = "strip_prefix", prefix = "/prue" }
```

- [ ] **Step 2: 修改 Docker Compose**

在 `infrastructure/docker-compose.yml` 的 `iam-service` 后添加：

```yaml
  prue_backend:
    build:
      context: ..
      dockerfile: services/prue_backend/Dockerfile
    ports:
      - "6004:8080"
    environment:
      SERVER_HOST: "0.0.0.0"
      SERVER_PORT: "8080"
      DATABASE_URL: "mysql://root:rootpass@mysql:3306/superpowers"
      RUST_LOG: "info"
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
    networks:
      - superpowers
```

- [ ] **Step 3: 校验路由**

仓库规则要求路由变更后校验。

在真实 PowerShell（vcvarsall x64）运行：

```powershell
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes
```

期望：路由校验通过，无 duplicate match pattern。

---

## Task 6: 移除 iam-service 菜单职责

**Files:**
- Modify: `services/iam-service/src/routes/mod.rs`
- Modify: `services/iam-service/src/dto/mod.rs`
- Modify: `services/iam-service/src/models/mod.rs`
- Modify: `services/iam-service/src/service/mod.rs`
- Modify: `services/iam-service/src/openapi.rs`
- Modify: `services/iam-service/README.md`
- Modify: `services/iam-service/AGENTS.md`
- Modify: `services/iam-service/tests/swagger_test.rs`
- Modify: `docker/config/api-gateway/routes/iam.router.toml`
- Delete: `services/iam-service/src/routes/menus.rs`
- Delete: `services/iam-service/src/dto/menu.rs`
- Delete: `services/iam-service/src/models/menu.rs`
- Delete: `services/iam-service/src/service/menu_svc.rs`

- [ ] **Step 1: 修改 routes/mod.rs**

把：

```rust
pub mod menus;
```

删除。

把：

```rust
        .merge(menus::routes(state.clone()))
```

删除。

- [ ] **Step 2: 修改 dto/mod.rs**

删除：

```rust
pub mod menu;
pub use menu::*;
```

- [ ] **Step 3: 修改 models/mod.rs**

删除：

```rust
pub mod menu;
pub use menu::Menu;
```

- [ ] **Step 4: 修改 service/mod.rs**

删除：

```rust
pub mod menu_svc;
pub use menu_svc::MenuService;
```

- [ ] **Step 5: 修改 openapi.rs**

把：

```rust
use crate::routes::{auth, menus, users};
```

改为：

```rust
use crate::routes::{auth, users};
```

删除 paths 中：

```rust
        menus::list_menus,
        menus::create_menu,
        menus::get_menu,
        menus::update_menu,
        menus::delete_menu,
```

删除 schemas 中：

```rust
        dto::CreateMenuRequest,
        dto::UpdateMenuRequest,
        dto::MenuResponse,
        dto::MenuListResponse,
```

删除 tags 中：

```rust
        (name = "menus", description = "Menu management"),
```

把 info description 中的 `menu` 描述移除。

- [ ] **Step 6: 修改 swagger_test.rs**

删除：

```rust
    assert!(paths.contains_key("/iam/menus"));
    assert!(paths.contains_key("/iam/menus/{menu_id}"));
```

保留 auth 和 users 断言。

- [ ] **Step 7: 修改 iam README**

从 `services/iam-service/README.md` 删除菜单管理段落：

```markdown
- **菜单管理** (`/iam/menus/*`)
  - 菜单树结构
  - 菜单权限关联
```

把开头描述改为：

```markdown
统一身份与访问管理服务，整合认证、用户、角色、权限、部门、Token 管理能力。
```

- [ ] **Step 8: 修改 iam AGENTS**

从 `services/iam-service/AGENTS.md` 删除菜单端点段落：

```markdown
### 菜单 /iam/menus/*
| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/menus | GET | 查询菜单列表（树形） |
| /iam/menus | POST | 创建菜单 |
| /iam/menus/:id | GET | 获取菜单详情 |
| /iam/menus/:id | PUT | 更新菜单 |
| /iam/menus/:id | DELETE | 删除菜单 |
```

- [ ] **Step 9: 修改 iam 网关路由**

从 `docker/config/api-gateway/routes/iam.router.toml` 删除：

```toml
[[route]]
match = "/iam/menus/**"
service = "iam-service"
public = false
```

- [ ] **Step 10: 删除 iam 菜单源码**

删除：

```text
services/iam-service/src/routes/menus.rs
services/iam-service/src/dto/menu.rs
services/iam-service/src/models/menu.rs
services/iam-service/src/service/menu_svc.rs
```

使用 IDE 删除工具，不能用 shell 删除。

- [ ] **Step 11: 检查残留引用**

使用内容搜索检查：

```text
menus::
MenuService
CreateMenuRequest
MenuListResponse
/iam/menus
```

期望：业务源码中无残留引用。

文档历史 spec/plan 可保留。

---

## Task 7: 验证和格式化

**Files:**
- All changed files

- [ ] **Step 1: Rust 格式检查**

在真实 PowerShell（vcvarsall x64）运行：

```powershell
cargo fmt --check
```

期望：格式通过。

如失败，运行：

```powershell
cargo fmt
```

再运行：

```powershell
cargo fmt --check
```

- [ ] **Step 2: prue_backend 测试**

在真实 PowerShell（vcvarsall x64）运行：

```powershell
cargo test -p prue_backend
```

期望：测试通过。

- [ ] **Step 3: iam-service OpenAPI 测试**

在真实 PowerShell（vcvarsall x64）运行：

```powershell
cargo test -p iam-service --test swagger_test
```

期望：测试通过，且不再断言 `/iam/menus*`。

- [ ] **Step 4: clippy 检查**

在真实 PowerShell（vcvarsall x64）运行：

```powershell
cargo clippy --all-targets -- -D warnings
```

期望：无 warning。

- [ ] **Step 5: 路由校验**

在真实 PowerShell（vcvarsall x64）运行：

```powershell
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes
```

期望：通过。

- [ ] **Step 6: 变更检查**

运行只读命令：

```powershell
git status --short
```

确认没有把 `private.pem`、构建产物、报告文件加入变更。

---

## 自检清单

- spec 要求的独立 `prue_backend`：Task 1 覆盖。
- JSON 版本化持久化：Task 2、Task 3 覆盖。
- active 版本输出 `/get-async-routes`：Task 4 覆盖。
- 用户、角色只作为字符串数组：Task 2 覆盖。
- 不关联 iam-service：Task 3 不引入 iam 依赖。
- 移除 iam 菜单职责：Task 6 覆盖。
- 网关路由：Task 5、Task 7 覆盖。
- Docker 端口 6004：Task 5 覆盖。
- pure-admin 文档链接写入 AGENTS：Task 1 覆盖。

## 执行约束

- 不在 IDE 终端直接运行 cargo。
- cargo 命令必须在真实 PowerShell（vcvarsall x64）运行。
- 不主动 commit。
- 不主动 push。
- 删除文件使用 IDE 删除工具。
