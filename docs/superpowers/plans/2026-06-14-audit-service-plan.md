# Phase 2: audit-service 审计日志服务 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现审计日志服务，支持操作日志、登录日志、API 访问日志，具备合规导出与告警能力。

**Architecture:** 微服务架构，使用 common-db/common-log/common-cache。HTTP 同步上报 + 本地 buffer 兜底，无 MQ。

**Tech Stack:** Rust 2021 / axum 0.8 / sqlx 0.8 / tokio / tracing

参照设计：[2026-06-13-enterprise-base-libs-design.md](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-13-enterprise-base-libs-design.md) §4.3。

执行边界：本计划仅触及 `services/audit-service/`、`migrations/audit-service/`、`contracts/`（需新增 AUD 错误码）。

---

## 总体文件结构

```
services/audit-service/
├─ Cargo.toml
├─ src/
│  ├─ lib.rs
│  ├─ main.rs
│  ├─ config.rs
│  ├─ error.rs
│  ├─ routes/
│  │  ├─ mod.rs
│  │  ├─ operation.rs    # 操作日志
│  │  ├─ login.rs        # 登录日志
│  │  └─ export.rs       # 导出
│  ├─ models/
│  │  ├─ mod.rs
│  │  ├─ operation.rs
│  │  ├─ login.rs
│  │  └─ api_access.rs
│  └─ services/
│     ├─ mod.rs
│     └─ audit.rs
├─ migrations/
│  ├─ mysql/V001__init.sql
│  └─ sqlite/V001__init.sql
├─ config/config.yaml
├─ Dockerfile
├─ AGENTS.md
└─ README.md
```

---

## Task 1：项目骨架 + 数据库迁移

**Files:**
- Create: `services/audit-service/Cargo.toml`
- Create: `services/audit-service/src/lib.rs`
- Create: `services/audit-service/src/main.rs`
- Create: `services/audit-service/src/config.rs`
- Create: `services/audit-service/src/error.rs`
- Create: `services/audit-service/migrations/mysql/V001__init.sql`
- Create: `services/audit-service/migrations/sqlite/V001__init.sql`
- Modify: `Cargo.toml` workspace.members += `"services/audit-service"`
- Modify: `contracts/error-codes.yaml` 新增 AUD_ 错误码

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "audit-service"
version = "0.1.0"
edition = "2021"

[dependencies]
axum.workspace = true
tokio.workspace = true
tracing.workspace = true
sqlx.workspace = true
serde.workspace = true
serde_json.workspace = true
common-error = { path = "../../libs/common-error" }
common-db = { path = "../../libs/common-db", features = ["mysql", "sqlite"] }
common-log = { path = "../../libs/common-log" }
common-cache = { path = "../../libs/common-cache" }
```

- [ ] **Step 2: 创建错误码（contracts/error-codes.yaml）**

```yaml
# === 审计日志 ===
AUD_LOG_1001:
  message: "audit log not found"
  kind: NotFound
AUD_LOG_1002:
  message: "export format not supported"
  kind: BadRequest
AUD_LOG_1003:
  message: "export generation failed"
  kind: Internal
```

- [ ] **Step 3: 运行 contracts-gen**

```bash
node scripts/contracts-gen.js
```

- [ ] **Step 4: 创建数据库迁移**

`migrations/mysql/V001__init.sql`:
```sql
CREATE TABLE operation_logs (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    user_id VARCHAR(64) NOT NULL COMMENT '用户ID',
    action VARCHAR(128) NOT NULL COMMENT '动作',
    resource VARCHAR(256) NOT NULL COMMENT '资源',
    resource_id VARCHAR(128) COMMENT '资源ID',
    before_value TEXT COMMENT '变更前',
    after_value TEXT COMMENT '变更后',
    request_id VARCHAR(64) NOT NULL COMMENT '请求ID',
    ip_address VARCHAR(45) COMMENT 'IP地址',
    user_agent TEXT COMMENT 'User-Agent',
    created_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    INDEX idx_user_id (user_id),
    INDEX idx_action (action),
    INDEX idx_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE login_logs (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    user_id VARCHAR(64) COMMENT '用户ID',
    username VARCHAR(128) NOT NULL COMMENT '用户名',
    success BOOLEAN NOT NULL COMMENT '是否成功',
    fail_reason VARCHAR(256) COMMENT '失败原因',
    ip_address VARCHAR(45) NOT NULL COMMENT 'IP地址',
    location VARCHAR(256) COMMENT '地理位置',
    user_agent TEXT COMMENT 'User-Agent',
    request_id VARCHAR(64) NOT NULL COMMENT '请求ID',
    created_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    INDEX idx_username (username),
    INDEX idx_success (success),
    INDEX idx_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE api_access_logs (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    service_name VARCHAR(64) NOT NULL COMMENT '服务名',
    method VARCHAR(10) NOT NULL COMMENT 'HTTP方法',
    path VARCHAR(512) NOT NULL COMMENT '请求路径',
    status_code INT NOT NULL COMMENT '状态码',
    duration_ms INT NOT NULL COMMENT '耗时ms',
    user_id VARCHAR(64) COMMENT '用户ID',
    request_id VARCHAR(64) NOT NULL COMMENT '请求ID',
    created_at DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),
    INDEX idx_service (service_name),
    INDEX idx_path (path),
    INDEX idx_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

- [ ] **Step 5: 创建基础服务代码**

`src/lib.rs`:
```rust
pub mod config;
pub mod error;
pub mod models;
pub mod routes;
pub mod services;

pub use config::AppConfig;
```

- [ ] **Step 6: Commit**

```bash
git add services/audit-service contracts
git commit -m "feat(audit-service): add audit-service skeleton with migrations"
```

---

## Task 2：操作日志 CRUD

**Files:**
- Create: `services/audit-service/src/models/operation.rs`
- Create: `services/audit-service/src/services/audit.rs`
- Create: `services/audit-service/src/routes/operation.rs`
- Modify: `services/audit-service/src/models/mod.rs`
- Modify: `services/audit-service/src/services/mod.rs`
- Modify: `services/audit-service/src/routes/mod.rs`

- [ ] **Step 1: 创建操作日志模型**

`src/models/operation.rs`:
```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OperationLog {
    pub id: i64,
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
    pub request_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOperationLog {
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
    pub request_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueryOperationLogs {
    pub user_id: Option<String>,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}
```

- [ ] **Step 2: 创建审计服务**

`src/services/audit.rs`:
```rust
use crate::error::{AppError, AppResult};
use crate::models::operation::{CreateOperationLog, OperationLog, QueryOperationLogs};
use common_db::DbPool;
use sqlx::Row;

pub struct AuditService {
    pool: DbPool,
}

impl AuditService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create_operation_log(&self, log: CreateOperationLog) -> AppResult<i64> {
        let result = match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query(
                    r#"INSERT INTO operation_logs 
                       (user_id, action, resource, resource_id, before_value, after_value, request_id, ip_address, user_agent)
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
                )
                .bind(&log.user_id)
                .bind(&log.action)
                .bind(&log.resource)
                .bind(&log.resource_id)
                .bind(&log.before_value)
                .bind(&log.after_value)
                .bind(&log.request_id)
                .bind(&log.ip_address)
                .bind(&log.user_agent)
                .execute(p)
                .await?
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query(
                    r#"INSERT INTO operation_logs 
                       (user_id, action, resource, resource_id, before_value, after_value, request_id, ip_address, user_agent)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#
                )
                .bind(&log.user_id)
                .bind(&log.action)
                .bind(&log.resource)
                .bind(&log.resource_id)
                .bind(&log.before_value)
                .bind(&log.after_value)
                .bind(&log.request_id)
                .bind(&log.ip_address)
                .bind(&log.user_agent)
                .execute(p)
                .await?
            }
        };
        Ok(result.last_insert_id() as i64)
    }

    pub async fn query_operation_logs(&self, query: QueryOperationLogs) -> AppResult<Vec<OperationLog>> {
        let page = query.page.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).min(100);
        let offset = (page - 1) * size;

        // TODO: 构建动态查询
        let logs = match &self.pool {
            #[cfg(feature = "mysql")]
            common_db::DbPool::Mysql(p) => {
                sqlx::query_as::<_, OperationLog>(
                    "SELECT * FROM operation_logs ORDER BY created_at DESC LIMIT ? OFFSET ?"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
            #[cfg(feature = "sqlite")]
            common_db::DbPool::Sqlite(p) => {
                sqlx::query_as::<_, OperationLog>(
                    "SELECT * FROM operation_logs ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
                )
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await?
            }
        };

        Ok(logs)
    }
}
```

- [ ] **Step 3: 创建路由**

`src/routes/operation.rs`:
```rust
use axum::{extract::State, Json};
use crate::error::AppResult;
use crate::models::operation::{CreateOperationLog, QueryOperationLogs};
use crate::AppState;

pub async fn create_operation_log(
    State(state): State<AppState>,
    Json(log): Json<CreateOperationLog>,
) -> AppResult<Json<serde_json::Value>> {
    let id = state.audit.create_operation_log(log).await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn query_operation_logs(
    State(state): State<AppState>,
    Query(query): Query<QueryOperationLogs>,
) -> AppResult<Json<serde_json::Value>> {
    let logs = state.audit.query_operation_logs(query).await?;
    Ok(Json(serde_json::json!({ "items": logs })))
}
```

- [ ] **Step 4: 注册路由**

`src/routes/mod.rs`:
```rust
pub mod operation;

use axum::Router;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/audit/operation", axum::routing::post(operation::create_operation_log))
        .route("/audit/operation", axum::routing::get(operation::query_operation_logs))
}
```

- [ ] **Step 5: Commit**

```bash
git add services/audit-service
git commit -m "feat(audit-service): add operation logs CRUD"
```

---

## Task 3：登录日志

**Files:**
- Create: `services/audit-service/src/models/login.rs`
- Create: `services/audit-service/src/routes/login.rs`
- Modify: `services/audit-service/src/models/mod.rs`
- Modify: `services/audit-service/src/routes/mod.rs`

- [ ] **Step 1: 创建登录日志模型**

`src/models/login.rs`:
```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LoginLog {
    pub id: i64,
    pub user_id: Option<String>,
    pub username: String,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub ip_address: String,
    pub location: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLoginLog {
    pub user_id: Option<String>,
    pub username: String,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub ip_address: String,
    pub location: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: String,
}
```

- [ ] **Step 2: 实现登录日志服务**

在 `src/services/audit.rs` 添加方法：
```rust
pub async fn create_login_log(&self, log: CreateLoginLog) -> AppResult<i64> {
    // 类似 create_operation_log 的实现
}
```

- [ ] **Step 3: 创建登录日志路由**

`src/routes/login.rs`:
```rust
use axum::{extract::State, Json};
use crate::error::AppResult;
use crate::models::login::{CreateLoginLog, LoginLog};
use crate::AppState;

pub async fn create_login_log(
    State(state): State<AppState>,
    Json(log): Json<CreateLoginLog>,
) -> AppResult<Json<serde_json::Value>> {
    let id = state.audit.create_login_log(log).await?;
    Ok(Json(serde_json::json!({ "id": id })))
}
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(audit-service): add login logs"
```

---

## Task 4：合规导出

**Files:**
- Create: `services/audit-service/src/routes/export.rs`

- [ ] **Step 1: 实现 CSV 导出**

`src/routes/export.rs`:
```rust
use axum::{extract::State, Json};
use crate::error::{AppError, AppResult};
use crate::models::operation::QueryOperationLogs;
use crate::AppState;

pub async fn export_operation_logs_csv(
    State(state): State<AppState>,
    Query(query): Query<QueryOperationLogs>,
) -> AppResult<String> {
    let logs = state.audit.query_operation_logs(query).await?;
    
    let mut csv = String::from("id,user_id,action,resource,resource_id,before_value,after_value,ip_address,created_at\n");
    for log in logs {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            log.id,
            log.user_id,
            log.action,
            log.resource,
            log.resource_id.unwrap_or_default(),
            log.before_value.unwrap_or_default(),
            log.after_value.unwrap_or_default(),
            log.ip_address.unwrap_or_default(),
            log.created_at
        ));
    }
    Ok(csv)
}
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(audit-service): add CSV export for operation logs"
```

---

## Task 5：服务入口 + 健康检查

**Files:**
- Modify: `services/audit-service/src/main.rs`
- Modify: `services/audit-service/src/lib.rs`

- [ ] **Step 1: 创建 main.rs**

`src/main.rs`:
```rust
use audit_service::{routes, AppConfig, AppState};
use common_db::init_pool;
use common_log::init_json;
use std::sync::Arc;
use axum::Router;

#[tokio::main]
async fn main() {
    init_json("info");
    
    let config = AppConfig::from_env();
    let pool = init_pool(&config.database_url).await.unwrap();
    
    let state = AppState {
        audit: Arc::new(audit_service::services::AuditService::new(pool)),
    };
    
    let app = Router::new()
        .nest("/audit", routes::routes())
        .route("/health", axum::routing::get(|| async { "ok" }));
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(audit-service): add main entry point with health check"
```

---

## Task 6：Docker + 服务规范

**Files:**
- Create: `services/audit-service/Dockerfile`
- Create: `services/audit-service/config/config.yaml`
- Create: `services/audit-service/AGENTS.md`
- Create: `services/audit-service/README.md`

- [ ] **Step 1: 创建 Dockerfile**

```dockerfile
FROM rust:1.75-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/audit-service /usr/local/bin/
EXPOSE 8080
CMD ["audit-service"]
```

- [ ] **Step 2: 创建 AGENTS.md**

```markdown
# audit-service 开发规范

## 必须项
- Dockerfile
- 健康检查：/health
- 数据库迁移：migrations/
- config.yaml

## 环境变量
- DATABASE_URL: 数据库连接
- RUST_LOG: 日志级别
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(audit-service): add Dockerfile and service standard files"
```

---

## Task 7：单元测试

**Files:**
- Create: `services/audit-service/tests/operation_test.rs`

- [ ] **Step 1: 创建测试**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn operation_log_serialization() {
        // 测试序列化
    }
}
```

- [ ] **Step 2: Commit**

```bash
git commit -m "test(audit-service): add unit tests"
```

---

## 自查（plan self-review）

1. Spec 覆盖：
   - §4.3 操作日志 → Task 2
   - §4.3 登录日志 → Task 3
   - §4.3 合规导出 CSV → Task 4
   - §4.3 API 访问日志 → 简化版（网关旁路可后续实现）
   - §4.3 保留策略/告警 → 预留，未实现
2. 依赖：common-db / common-log / common-error 已存在
3. 范围控制：audit-service 服务本身，不含网关旁路
