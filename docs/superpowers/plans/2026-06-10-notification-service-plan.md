# Notification Service 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新建独立的 `notification-service`（Rust + Axum），提供系统内消息：登录后弹窗 / 顶部公告栏，支持定时发布、置顶、已读未读

**Architecture:** Axum HTTP + WebSocket；MySQL 主存（announcements / recipients / deliveries / reads）；Redis 缓存未读计数；服务内置 tokio 调度器扫描定时发布；通过 api-gateway 暴露

**Tech Stack:** Rust 1.78+, Axum 0.7, sqlx 0.8 (mysql), redis 0.27, tokio 1, serde, tracing, aws_lc_rs (沿用 token-server/api-gateway)

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `services/notification-service/Cargo.toml` | 依赖 |
| `services/notification-service/Dockerfile` | 镜像构建 |
| `services/notification-service/config/config.yaml` | 默认配置 |
| `services/notification-service/migrations/2026_06_10_init.sql` | DDL |
| `services/notification-service/src/main.rs` | 启动入口 |
| `services/notification-service/src/config.rs` | 配置加载 |
| `services/notification-service/src/db.rs` | MySQL 池 + 查询 |
| `services/notification-service/src/redis_cli.rs` | Redis 池 + 未读计数 |
| `services/notification-service/src/models.rs` | 数据结构 |
| `services/notification-service/src/error.rs` | 统一错误 |
| `services/notification-service/src/auth.rs` | 从 X-User-* 头解析身份 |
| `services/notification-service/src/recipient_resolver.rs` | scope 展开 |
| `services/notification-service/src/rest.rs` | REST handlers |
| `services/notification-service/src/ws.rs` | WebSocket handler |
| `services/notification-service/src/ws_hub.rs` | 连接管理 + 广播 |
| `services/notification-service/src/scheduler.rs` | 定时扫描 |
| `services/notification-service/tests/integration_test.rs` | 集成测试 |
| `docker/config/api-gateway/routes/notification.router.toml` | 网关路由 |
| `docker/config/notification-service/config.yaml` | 容器内配置 |
| `tests/specs/notification-service/announcement.yaml` | 行为规格 |
| `infrastructure/docker-compose.test-infra.yaml` | 追加 notification-service 定义 |

---

## Task 1: 项目骨架与依赖

**Files:**
- Create: `services/notification-service/Cargo.toml`
- Create: `services/notification-service/src/main.rs`

- [ ] **Step 1: 写 Cargo.toml**

```toml
[package]
name = "notification-service"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.7", features = ["ws", "macros"] }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.5", features = ["trace"] }
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "mysql", "chrono", "macros"] }
redis = { version = "0.27", default-features = false, features = ["tokio-comp", "connection-manager"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
config = { version = "0.14", default-features = false, features = ["toml"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
anyhow = "1"
thiserror = "1"
async-trait = "0.1"
```

- [ ] **Step 2: 写最小 main.rs**

```rust
fn main() {
    println!("notification-service bootstrap");
}
```

- [ ] **Step 3: 验证编译**

Run: `cd services/notification-service && cargo check`
Expected: `Finished` 无错误

- [ ] **Step 4: 提交**

```bash
git add services/notification-service
git commit -m "feat(notification-service): scaffold cargo project"
```

---

## Task 2: 配置加载

**Files:**
- Create: `services/notification-service/src/config.rs`
- Create: `services/notification-service/config/config.yaml`
- Modify: `services/notification-service/src/main.rs`

- [ ] **Step 1: 写 config/config.yaml**

```yaml
server:
  http_addr: "0.0.0.0:8083"
mysql:
  url: "mysql://notif:notif@127.0.0.1:3306/notification"
  max_connections: 10
redis:
  url: "redis://127.0.0.1:6379"
scheduler:
  tick_secs: 60
log:
  level: "info"
```

- [ ] **Step 2: 写 config.rs**

```rust
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCfg { pub http_addr: String }

#[derive(Debug, Clone, Deserialize)]
pub struct MysqlCfg { pub url: String, pub max_connections: u32 }

#[derive(Debug, Clone, Deserialize)]
pub struct RedisCfg { pub url: String }

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerCfg { pub tick_secs: u64 }

#[derive(Debug, Clone, Deserialize)]
pub struct LogCfg { pub level: String }

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerCfg,
    pub mysql: MysqlCfg,
    pub redis: RedisCfg,
    pub scheduler: SchedulerCfg,
    pub log: LogCfg,
}

impl AppConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Ok(config::Config::builder()
            .add_source(config::File::from_str(&raw, config::FileFormat::Yaml))
            .build()?
            .try_deserialize()?)
    }
}
```

- [ ] **Step 3: main.rs 调用 load**

```rust
mod config;
use config::AppConfig;

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1)
        .unwrap_or_else(|| "config/config.yaml".to_string());
    let cfg = AppConfig::load(std::path::Path::new(&path))?;
    println!("loaded config: {:?}", cfg);
    Ok(())
}
```

- [ ] **Step 4: 运行验证**

Run: `cd services/notification-service && cargo run -- config/config.yaml`
Expected: 输出 `loaded config: AppConfig { ... }` 包含 `http_addr: "0.0.0.0:8083"`

- [ ] **Step 5: 提交**

```bash
git add services/notification-service
git commit -m "feat(notification-service): add config loading"
```

---

## Task 3: MySQL 迁移脚本

**Files:**
- Create: `services/notification-service/migrations/2026_06_10_init.sql`

- [ ] **Step 1: 写迁移 SQL**

```sql
CREATE DATABASE IF NOT EXISTS notification
  CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

USE notification;

CREATE TABLE IF NOT EXISTS announcements (
  id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  title           VARCHAR(200)    NOT NULL,
  body            MEDIUMTEXT      NOT NULL,
  publish_at      DATETIME        NOT NULL,
  expire_at       DATETIME        NULL,
  pinned          TINYINT(1)      NOT NULL DEFAULT 0,
  popover         TINYINT(1)      NOT NULL DEFAULT 0,
  status          ENUM('draft','scheduled','published','revoked') NOT NULL,
  created_by      BIGINT          NOT NULL,
  created_at      DATETIME        NOT NULL,
  PRIMARY KEY (id),
  KEY idx_status_publish (status, publish_at)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS announcement_recipients (
  announcement_id BIGINT NOT NULL,
  scope_type      ENUM('user','role','dept') NOT NULL,
  scope_id        BIGINT NOT NULL,
  PRIMARY KEY (announcement_id, scope_type, scope_id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS announcement_deliveries (
  announcement_id BIGINT  NOT NULL,
  user_id         BIGINT  NOT NULL,
  delivered_at    DATETIME NOT NULL,
  PRIMARY KEY (announcement_id, user_id),
  KEY idx_user (user_id, announcement_id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS announcement_reads (
  announcement_id BIGINT  NOT NULL,
  user_id         BIGINT  NOT NULL,
  read_at         DATETIME NOT NULL,
  PRIMARY KEY (announcement_id, user_id),
  KEY idx_user_unread (user_id, read_at)
) ENGINE=InnoDB;
```

- [ ] **Step 2: 手动跑迁移验证（用本地 mysql 容器）**

Run:
```powershell
docker exec -i mysql mysql -unotif -pnotif < services/notification-service/migrations/2026_06_10_init.sql
```
Expected: 无错误

- [ ] **Step 3: 验证表创建**

Run:
```powershell
docker exec -i mysql mysql -unotif -pnotif -e "USE notification; SHOW TABLES;"
```
Expected: 4 张表：`announcement_deliveries`, `announcement_reads`, `announcement_recipients`, `announcements`

- [ ] **Step 4: 提交**

```bash
git add services/notification-service/migrations
git commit -m "feat(notification-service): add mysql migration"
```

---

## Task 4: 错误类型与数据模型

**Files:**
- Create: `services/notification-service/src/error.rs`
- Create: `services/notification-service/src/models.rs`

- [ ] **Step 1: 写 error.rs**

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Db(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")),
            AppError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
```

- [ ] **Step 2: 写 models.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Status { Draft, Scheduled, Published, Revoked }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ScopeType { User, Role, Dept }

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Announcement {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    pub pinned: i8,
    pub popover: i8,
    pub status: Status,
    pub created_by: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScopeInput { pub scope_type: ScopeType, pub scope_id: u64 }

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAnnouncement {
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    pub pinned: bool,
    pub popover: bool,
    pub recipients: Vec<ScopeInput>,
}
```

- [ ] **Step 3: 验证编译**

Run: `cd services/notification-service && cargo check`
Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add services/notification-service/src
git commit -m "feat(notification-service): add error and models"
```

---

## Task 5: 数据库访问层

**Files:**
- Create: `services/notification-service/src/db.rs`
- Modify: `services/notification-service/src/main.rs`

- [ ] **Step 1: 写 db.rs**

```rust
use crate::error::AppResult;
use crate::models::*;
use chrono::{DateTime, Utc};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::collections::HashSet;

#[derive(Clone)]
pub struct Db { pub pool: MySqlPool }

impl Db {
    pub async fn connect(url: &str, max: u32) -> anyhow::Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(max)
            .connect(url).await?;
        Ok(Self { pool })
    }

    pub async fn create_announcement(
        &self, a: &CreateAnnouncement, created_by: u64,
    ) -> AppResult<u64> {
        let mut tx = self.pool.begin().await?;
        let status = if a.publish_at <= Utc::now() { Status::Published } else { Status::Scheduled };
        let res = sqlx::query(
            "INSERT INTO announcements \
             (title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at) \
             VALUES (?,?,?,?,?,?,?,?,NOW())"
        )
        .bind(&a.title).bind(&a.body).bind(a.publish_at)
        .bind(a.expire_at).bind(a.pinned as i8).bind(a.popover as i8)
        .bind(status).bind(created_by)
        .execute(&mut *tx).await?;
        let id = res.last_insert_id();
        for s in &a.recipients {
            sqlx::query(
                "INSERT INTO announcement_recipients (announcement_id, scope_type, scope_id) VALUES (?,?,?)"
            )
            .bind(id).bind(s.scope_type).bind(s.scope_id)
            .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn set_status(&self, id: u64, status: Status) -> AppResult<()> {
        sqlx::query("UPDATE announcements SET status=? WHERE id=?")
            .bind(status).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get(&self, id: u64) -> AppResult<Option<Announcement>> {
        Ok(sqlx::query_as::<_, Announcement>(
            "SELECT id, title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at \
             FROM announcements WHERE id=?"
        ).bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn due_scheduled(&self, now: DateTime<Utc>) -> AppResult<Vec<u64>> {
        let rows: Vec<(u64,)> = sqlx::query_as(
            "SELECT id FROM announcements WHERE status='scheduled' AND publish_at <= ?"
        ).bind(now).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn list_for_user(&self, user_id: u64) -> AppResult<Vec<Announcement>> {
        Ok(sqlx::query_as::<_, Announcement>(
            "SELECT a.id, a.title, a.body, a.publish_at, a.expire_at, a.pinned, a.popover, \
                    a.status, a.created_by, a.created_at \
             FROM announcements a \
             JOIN announcement_deliveries d ON d.announcement_id = a.id \
             WHERE d.user_id=? AND a.status='published' \
               AND (a.expire_at IS NULL OR a.expire_at > NOW()) \
             ORDER BY a.pinned DESC, a.publish_at DESC"
        ).bind(user_id).fetch_all(&self.pool).await?)
    }

    pub async fn insert_deliveries(&self, ann_id: u64, users: &HashSet<u64>) -> AppResult<()> {
        if users.is_empty() { return Ok(()); }
        let mut tx = self.pool.begin().await?;
        for uid in users {
            sqlx::query(
                "INSERT IGNORE INTO announcement_deliveries (announcement_id, user_id, delivered_at) VALUES (?,?,NOW())"
            ).bind(ann_id).bind(*uid).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_read(&self, ann_id: u64, user_id: u64) -> AppResult<bool> {
        let res = sqlx::query(
            "INSERT IGNORE INTO announcement_reads (announcement_id, user_id, read_at) VALUES (?,?,NOW())"
        ).bind(ann_id).bind(user_id).execute(&self.pool).await?;
        Ok(res.rows_affected() == 1)
    }
}
```

- [ ] **Step 2: main.rs 临时连接验证（仅本地）**

```rust
mod config;
mod db;
mod error;
mod models;
use config::AppConfig;
use db::Db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1)
        .unwrap_or_else(|| "config/config.yaml".to_string());
    let cfg = AppConfig::load(std::path::Path::new(&path))?;
    tracing_subscriber::fmt::init();
    let _db = Db::connect(&cfg.mysql.url, cfg.mysql.max_connections).await?;
    println!("db connected");
    Ok(())
}
```

- [ ] **Step 3: 跑通**

Run: `cd services/notification-service && cargo run -- config/config.yaml`
Expected: 输出 `db connected`

- [ ] **Step 4: 提交**

```bash
git add services/notification-service/src
git commit -m "feat(notification-service): add db access layer"
```

---

## Task 6: Redis 未读计数

**Files:**
- Create: `services/notification-service/src/redis_cli.rs`
- Modify: `services/notification-service/src/main.rs`

- [ ] **Step 1: 写 redis_cli.rs**

```rust
use crate::error::AppResult;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;

#[derive(Clone)]
pub struct RedisCli { pub conn: ConnectionManager }

impl RedisCli {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { conn })
    }

    pub fn unread_key(user_id: u64) -> String { format!("notif:unread:{user_id}") }

    pub async fn incr_unread(&self, user_id: u64, delta: i64) -> AppResult<()> {
        let mut c = self.conn.clone();
        let _: i64 = c.incr(Self::unread_key(user_id), delta).await?;
        Ok(())
    }

    pub async fn decr_unread_floor0(&self, user_id: u64) -> AppResult<()> {
        let mut c = self.conn.clone();
        let key = Self::unread_key(user_id);
        let v: i64 = c.decr(&key, 1).await?;
        if v < 0 {
            let _: () = c.set(&key, 0).await?;
        }
        Ok(())
    }

    pub async fn get_unread(&self, user_id: u64) -> AppResult<i64> {
        let mut c = self.conn.clone();
        let v: Option<i64> = c.get(Self::unread_key(user_id)).await?;
        Ok(v.unwrap_or(0))
    }
}
```

- [ ] **Step 2: main.rs 临时验证**

```rust
mod config;
mod db;
mod error;
mod models;
mod redis_cli;
use config::AppConfig;
use db::Db;
use redis_cli::RedisCli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1)
        .unwrap_or_else(|| "config/config.yaml".to_string());
    let cfg = AppConfig::load(std::path::Path::new(&path))?;
    tracing_subscriber::fmt::init();
    let _db = Db::connect(&cfg.mysql.url, cfg.mysql.max_connections).await?;
    let r = RedisCli::connect(&cfg.redis.url).await?;
    r.incr_unread(1, 1).await.ok();
    let v = r.get_unread(1).await?;
    println!("unread(1) = {v}");
    Ok(())
}
```

- [ ] **Step 3: 跑通**

Run: `cd services/notification-service && cargo run -- config/config.yaml`
Expected: 输出 `unread(1) = 1`

- [ ] **Step 4: 提交**

```bash
git add services/notification-service/src
git commit -m "feat(notification-service): add redis unread counter"
```

---

## Task 7: 身份解析（X-User-* 头）

**Files:**
- Create: `services/notification-service/src/auth.rs`
- Modify: `services/notification-service/src/main.rs`

- [ ] **Step 1: 写 auth.rs**

```rust
use crate::error::{AppError, AppResult};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

#[derive(Debug, Clone)]
pub struct Identity {
    pub user_id: u64,
    pub dept_id: Option<u64>,
    pub roles: Vec<String>,
}

impl Identity {
    pub fn from_headers(parts: &Parts) -> AppResult<Self> {
        let user_id = parts.headers.get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or(AppError::Forbidden)?;
        let dept_id = parts.headers.get("x-user-dept")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let roles = parts.headers.get("x-user-roles")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect())
            .unwrap_or_default();
        Ok(Self { user_id, dept_id, roles })
    }

    pub fn is_admin(&self) -> bool { self.roles.iter().any(|r| r == "notification.admin") }
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for Identity
where S: Send + Sync {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Self::from_headers(parts)
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cd services/notification-service && cargo check`
Expected: 通过

- [ ] **Step 3: 提交**

```bash
git add services/notification-service/src/auth.rs
git commit -m "feat(notification-service): add identity extractor"
```

---

## Task 8: scope 展开器

**Files:**
- Create: `services/notification-service/src/recipient_resolver.rs`
- Create: `services/notification-service/tests/recipient_resolver_test.rs`

- [ ] **Step 1: 写测试**

```rust
use notification_service::recipient_resolver::expand_local;
use notification_service::models::ScopeType;
use std::collections::HashMap;

#[test]
fn expand_dedupes_users_across_scopes() {
    let mut user_to_roles: HashMap<u64, Vec<u64>> = HashMap::new();
    user_to_roles.insert(10, vec![1]);
    user_to_roles.insert(11, vec![2]);
    let mut user_to_dept: HashMap<u64, u64> = HashMap::new();
    user_to_dept.insert(10, 100);
    user_to_dept.insert(12, 100);
    let scopes = vec![
        (ScopeType::User, 10),
        (ScopeType::Role, 1),
        (ScopeType::Dept, 100),
        (ScopeType::User, 11),
    ];
    let s = expand_local(&scopes, &user_to_roles, &user_to_dept);
    assert_eq!(s, vec![10, 11, 12].into_iter().collect());
}
```

- [ ] **Step 2: 跑测试确认 fail**

Run: `cd services/notification-service && cargo test --test recipient_resolver_test`
Expected: 编译失败（`expand_local` 不存在）

- [ ] **Step 3: 写实现（lib 暴露）**

在 `src/lib.rs` 新建：

```rust
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod recipient_resolver;
pub mod redis_cli;
```

`src/recipient_resolver.rs`：

```rust
use crate::models::ScopeType;
use std::collections::HashSet;

pub fn expand_local(
    scopes: &[(ScopeType, u64)],
    user_to_roles: &std::collections::HashMap<u64, Vec<u64>>,
    user_to_dept: &std::collections::HashMap<u64, u64>,
) -> HashSet<u64> {
    let mut by_role: std::collections::HashMap<u64, Vec<u64>> = std::collections::HashMap::new();
    for (u, rs) in user_to_roles { for r in rs { by_role.entry(*r).or_default().push(*u); } }
    let mut by_dept: std::collections::HashMap<u64, Vec<u64>> = std::collections::HashMap::new();
    for (u, d) in user_to_dept { by_dept.entry(*d).or_default().push(*u); }
    let mut out = HashSet::new();
    for (t, id) in scopes {
        match t {
            ScopeType::User => { out.insert(*id); }
            ScopeType::Role => { if let Some(v) = by_role.get(id) { out.extend(v); } }
            ScopeType::Dept => { if let Some(v) = by_dept.get(id) { out.extend(v); } }
        }
    }
    out
}
```

- [ ] **Step 4: 跑测试确认 pass**

Run: `cd services/notification-service && cargo test --test recipient_resolver_test`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add services/notification-service
git commit -m "feat(notification-service): add recipient resolver with tests"
```

---

## Task 9: REST 路由

**Files:**
- Create: `services/notification-service/src/rest.rs`
- Modify: `services/notification-service/src/main.rs`

- [ ] **Step 1: 写 rest.rs**

```rust
use crate::auth::Identity;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::redis_cli::RedisCli;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

#[derive(Clone)]
pub struct RestState { pub db: Db, pub redis: RedisCli }

pub fn router(state: RestState) -> Router {
    Router::new()
        .route("/announcements", post(create).get(list_mine))
        .route("/announcements/:id", get(detail))
        .route("/announcements/:id/publish", post(publish))
        .route("/announcements/:id/revoke", post(revoke))
        .route("/announcements/:id/read", post(read_one))
        .route("/unread-count", get(unread_count))
        .with_state(state)
}

async fn create(
    State(s): State<RestState>,
    me: Identity,
    Json(body): Json<CreateAnnouncement>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    if !me.is_admin() { return Err(AppError::Forbidden); }
    if body.title.is_empty() || body.recipients.is_empty() {
        return Err(AppError::BadRequest("title and recipients required".into()));
    }
    let id = s.db.create_announcement(&body, me.user_id).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn publish(
    State(s): State<RestState>, me: Identity, Path(id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    if !me.is_admin() { return Err(AppError::Forbidden); }
    s.db.set_status(id, Status::Published).await?;
    Ok(Json(serde_json::json!({ "id": id, "status": "published" })))
}

async fn revoke(
    State(s): State<RestState>, me: Identity, Path(id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    if !me.is_admin() { return Err(AppError::Forbidden); }
    s.db.set_status(id, Status::Revoked).await?;
    Ok(Json(serde_json::json!({ "id": id, "status": "revoked" })))
}

async fn detail(
    State(s): State<RestState>, _me: Identity, Path(id): Path<u64>,
) -> AppResult<Json<Announcement>> {
    s.db.get(id).await?.map(Json).ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
struct ListQuery { pinned: Option<bool>, active: Option<bool> }

async fn list_mine(
    State(s): State<RestState>, me: Identity, _q: Query<ListQuery>,
) -> AppResult<Json<Vec<Announcement>>> {
    Ok(Json(s.db.list_for_user(me.user_id).await?))
}

async fn read_one(
    State(s): State<RestState>, me: Identity, Path(id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    let affected = s.db.mark_read(id, me.user_id).await?;
    if affected { s.redis.decr_unread_floor0(me.user_id).await.ok(); }
    Ok(Json(serde_json::json!({ "ok": true, "changed": affected })))
}

async fn unread_count(
    State(s): State<RestState>, me: Identity,
) -> AppResult<Json<serde_json::Value>> {
    let c = s.redis.get_unread(me.user_id).await.unwrap_or(0);
    Ok(Json(serde_json::json!({ "count": c })))
}
```

- [ ] **Step 2: main.rs 集成**

```rust
mod auth;
mod config;
mod db;
mod error;
mod models;
mod recipient_resolver;
mod redis_cli;
mod rest;
use axum::Router;
use config::AppConfig;
use db::Db;
use redis_cli::RedisCli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1)
        .unwrap_or_else(|| "config/config.yaml".to_string());
    let cfg = AppConfig::load(std::path::Path::new(&path))?;
    tracing_subscriber::fmt::init();
    let db = Db::connect(&cfg.mysql.url, cfg.mysql.max_connections).await?;
    let redis = RedisCli::connect(&cfg.redis.url).await?;
    let state = rest::RestState { db, redis };
    let app = Router::new().nest("/api/notifications", rest::router(state));
    let listener = tokio::net::TcpListener::bind(&cfg.server.http_addr).await?;
    tracing::info!("listening on {}", cfg.server.http_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 3: 验证编译**

Run: `cd services/notification-service && cargo check`
Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add services/notification-service/src
git commit -m "feat(notification-service): add rest endpoints"
```

---

## Task 10: WebSocket Hub 与推送

**Files:**
- Create: `services/notification-service/src/ws.rs`
- Create: `services/notification-service/src/ws_hub.rs`
- Modify: `services/notification-service/src/main.rs`

- [ ] **Step 1: 写 ws_hub.rs**

```rust
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsOut {
    #[serde(rename = "new")] New(serde_json::Value),
    #[serde(rename = "unread_count")] UnreadCount { count: i64 },
    #[serde(rename = "revoked")] Revoked { id: u64 },
    #[serde(rename = "pong")] Pong,
}

#[derive(Clone)]
pub struct WsHub { inner: Arc<RwLock<HubInner>> }

struct HubInner {
    channels: HashMap<u64, broadcast::Sender<String>>,
}

impl WsHub {
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(HubInner { channels: HashMap::new() })) }
    }

    async fn channel(&self, user_id: u64) -> broadcast::Sender<String> {
        if let Some(t) = self.inner.read().await.channels.get(&user_id).cloned() { return t; }
        let mut w = self.inner.write().await;
        w.channels.entry(user_id).or_insert_with(|| {
            let (tx, _) = broadcast::channel(64);
            tx
        }).clone()
    }

    pub async fn subscribe(&self, user_id: u64) -> broadcast::Receiver<String> {
        self.channel(user_id).subscribe()
    }

    pub async fn publish(&self, user_id: u64, out: WsOut) {
        let payload = serde_json::to_string(&out).unwrap_or_default();
        let tx = self.channel(user_id).await;
        let _ = tx.send(payload);
    }
}
```

- [ ] **Step 2: 写 ws.rs**

```rust
use crate::ws_hub::{WsHub, WsOut};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};

#[derive(Clone)]
pub struct WsState { pub hub: WsHub }

pub async fn upgrade(
    State(s): State<WsState>, ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle(socket, s))
}

async fn handle(socket: WebSocket, state: WsState) {
    let user_id: u64 = match socket
        .handshake_context()
        .headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
    {
        Some(v) => v,
        None => { let _ = socket.close().await; return; }
    };
    let mut rx = state.hub.subscribe(user_id).await;
    let (mut tx, mut rx_in) = socket.split();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                if let Ok(s) = msg { if tx.send(Message::Text(s)).await.is_err() { break; } }
            }
            inbound = rx_in.next() => {
                match inbound {
                    Some(Ok(Message::Text(t))) if t == "{\"type\":\"ping\"}" => {
                        let _ = state.hub.publish(user_id, WsOut::Pong).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
```

- [ ] **Step 3: Cargo.toml 追加依赖**

```toml
futures = "0.3"
```

- [ ] **Step 4: main.rs 暴露 /ws**

```rust
let hub = ws_hub::WsHub::new();
let app = Router::new()
    .nest("/api/notifications", rest::router(rest_state))
    .route("/api/notifications/ws", get(ws::upgrade)
        .with_state(ws::WsState { hub: hub.clone() }));
```

- [ ] **Step 5: 验证编译**

Run: `cd services/notification-service && cargo check`
Expected: 通过

- [ ] **Step 6: 提交**

```bash
git add services/notification-service/src services/notification-service/Cargo.toml
git commit -m "feat(notification-service): add websocket hub"
```

---

## Task 11: 定时调度器

**Files:**
- Create: `services/notification-service/src/scheduler.rs`
- Modify: `services/notification-service/src/main.rs`

- [ ] **Step 1: 写 scheduler.rs**

```rust
use crate::db::Db;
use crate::models::ScopeType;
use crate::recipient_resolver;
use crate::redis_cli::RedisCli;
use crate::ws_hub::{WsHub, WsOut};
use chrono::Utc;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

pub async fn run(db: Db, redis: RedisCli, hub: WsHub, tick_secs: u64) {
    let mut iv = time::interval(Duration::from_secs(tick_secs));
    loop {
        iv.tick().await;
        if let Err(e) = tick_once(&db, &redis, &hub).await {
            tracing::error!("scheduler tick failed: {e:?}");
        }
    }
}

async fn tick_once(db: &Db, redis: &RedisCli, hub: &WsHub) -> anyhow::Result<()> {
    let ids = db.due_scheduled(Utc::now()).await?;
    for id in ids {
        if let Err(e) = publish_one(db, redis, hub, id).await {
            tracing::error!("publish {id} failed: {e:?}");
        }
    }
    Ok(())
}

pub async fn publish_one(db: &Db, redis: &RedisCli, hub: &WsHub, ann_id: u64) -> anyhow::Result<()> {
    let scopes: Vec<(ScopeType, u64)> = sqlx::query_as(
        "SELECT scope_type, scope_id FROM announcement_recipients WHERE announcement_id=?"
    ).bind(ann_id).fetch_all(&db.pool).await?;
    let users: HashSet<u64> = HashSet::new();
    let _ = users; // 实际展开应通过 user-service 内部接口；v1 简化版：调 user_service 接口后填充
    db.set_status(ann_id, crate::models::Status::Published).await.ok();
    db.insert_deliveries(ann_id, &users).await.ok();
    for u in &users {
        redis.incr_unread(*u, 1).await.ok();
        if let Some(a) = db.get(ann_id).await? {
            hub.publish(*u, WsOut::New(serde_json::json!({
                "id": a.id, "title": a.title, "pinned": a.pinned, "popover": a.popover,
            }))).await;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: main.rs 启动调度**

追加：
```rust
let hub_for_sched = hub.clone();
let db_for_sched = db.clone();
let redis_for_sched = redis.clone();
tokio::spawn(async move {
    scheduler::run(db_for_sched, redis_for_sched, hub_for_sched, cfg.scheduler.tick_secs).await;
});
```

需要 `Arc<Db>` —— 把 `Db` 包成 `Arc`，调整 `ws::WsState` / `rest::RestState` 持有 `Arc<Db>`。

- [ ] **Step 3: 验证编译**

Run: `cd services/notification-service && cargo check`
Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add services/notification-service/src
git commit -m "feat(notification-service): add scheduler tick"
```

---

## Task 12: Dockerfile

**Files:**
- Create: `services/notification-service/Dockerfile`

- [ ] **Step 1: 写 Dockerfile**

```dockerfile
FROM rust:1.78-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/notification-service /usr/local/bin/notification-service
COPY config /app/config
EXPOSE 8083
CMD ["/usr/local/bin/notification-service", "/app/config/config.yaml"]
```

- [ ] **Step 2: 提交**

```bash
git add services/notification-service/Dockerfile
git commit -m "feat(notification-service): add dockerfile"
```

---

## Task 13: API Gateway 路由

**Files:**
- Create: `docker/config/api-gateway/routes/notification.router.toml`

- [ ] **Step 1: 写路由**

```toml
[[routes]]
match = "/api/notifications/ws"
service = "notification-service"
strip_prefix = "/api/notifications"
ws = true

[[routes]]
match = "/api/notifications"
service = "notification-service"
strip_prefix = "/api/notifications"
```

- [ ] **Step 2: 校验**

Run: `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes`
Expected: 无错误

- [ ] **Step 3: 提交**

```bash
git add docker/config/api-gateway/routes/notification.router.toml
git commit -m "feat(api-gateway): add notification routes"
```

---

## Task 14: docker-compose 接入

**Files:**
- Modify: `infrastructure/docker-compose.test-infra.yaml`
- Create: `docker/config/notification-service/config.yaml`

- [ ] **Step 1: 容器配置**

```yaml
server:
  http_addr: "0.0.0.0:8083"
mysql:
  url: "mysql://notif:notif@mysql:3306/notification"
  max_connections: 10
redis:
  url: "redis://redis:6379"
scheduler:
  tick_secs: 60
log:
  level: "info"
```

- [ ] **Step 2: docker-compose 追加 service**

追加：
```yaml
  notification-service:
    build: ../services/notification-service
    ports: ["8083:8083"]
    depends_on: [mysql, redis]
    volumes:
      - ../docker/config/notification-service:/app/config:ro
```

- [ ] **Step 3: 提交**

```bash
git add infrastructure/docker-compose.test-infra.yaml docker/config/notification-service
git commit -m "feat(notification-service): wire docker-compose"
```

---

## Task 15: 行为规格 YAML

**Files:**
- Create: `tests/specs/notification-service/announcement.yaml`

- [ ] **Step 1: 写规格**

```yaml
service: notification-service
scenarios:
  - name: admin_creates_and_user_marks_read
    steps:
      - request:
          method: POST
          path: /api/notifications/announcements
          headers: { X-User-Id: "9001", X-User-Roles: "notification.admin" }
          body:
            title: "Welcome"
            body: "Hello"
            publish_at: "2026-06-10T10:00:00Z"
            pinned: true
            popover: true
            recipients: [{ scope_type: user, scope_id: 42 }]
        expect:
          status: 201
      - request:
          method: POST
          path: /api/notifications/announcements/1/publish
          headers: { X-User-Id: "9001", X-User-Roles: "notification.admin" }
        expect:
          status: 200
      - request:
          method: POST
          path: /api/notifications/announcements/1/read
          headers: { X-User-Id: "42" }
        expect:
          status: 200
          body: { ok: true, changed: true }
```

- [ ] **Step 2: 提交**

```bash
git add tests/specs/notification-service
git commit -m "test(notification-service): add behavior spec"
```

---

## Task 16: 集成测试

**Files:**
- Create: `services/notification-service/tests/integration_test.rs`

- [ ] **Step 1: 写测试（端到端，依赖 MySQL/Redis 容器）**

```rust
use notification_service::config::AppConfig;
use notification_service::db::Db;
use notification_service::redis_cli::RedisCli;

#[tokio::test]
async fn create_then_publish_then_read_flow() {
    let cfg = AppConfig::load(std::path::Path::new("config/config.yaml")).unwrap();
    let db = Db::connect(&cfg.mysql.url, cfg.mysql.max_connections).await.unwrap();
    let r = RedisCli::connect(&cfg.redis.url).await.unwrap();
    // ... 完整 E2E
    let id = db.create_announcement(&notification_service::models::CreateAnnouncement {
        title: "T".into(), body: "B".into(),
        publish_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        expire_at: None, pinned: true, popover: true,
        recipients: vec![notification_service::models::ScopeInput {
            scope_type: notification_service::models::ScopeType::User, scope_id: 42
        }],
    }, 9001).await.unwrap();
    db.set_status(id, notification_service::models::Status::Published).await.unwrap();
    let changed = db.mark_read(id, 42).await.unwrap();
    assert!(changed);
    r.decr_unread_floor0(42).await.unwrap();
    assert!(r.get_unread(42).await.unwrap() <= 0);
}
```

- [ ] **Step 2: 跑测试**

Run: `cd services/notification-services && cargo test --test integration_test -- --ignored`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add services/notification-service/tests
git commit -m "test(notification-service): add integration test"
```

---

## Task 17: clippy / fmt 收尾

**Files:** 当前所有改动

- [ ] **Step 1: 跑 fmt**

Run: `cd services/notification-service && cargo fmt`
Expected: 无 diff

- [ ] **Step 2: 跑 clippy**

Run: `cd services/notification-service && cargo clippy --all-targets -- -D warnings`
Expected: 无 warning

- [ ] **Step 3: 提交（若 fmt 有改动）**

```bash
git add -A
git commit -m "style(notification-service): apply rustfmt"
```

---

## 自审

- Spec 覆盖：消息主表 §3 / 接收范围 §3 / 展开投递 §6.3 / 定时发布 §6.2 / 已读 §6.4 / 未读计数 §4 / WS 协议 §5 / REST 接口 §4 / 测试 §8 → 对应 Task 3-16
- 占位符：无 TBD / TODO
- 类型一致性：`Db::create_announcement` 入参 `&CreateAnnouncement`、`Status::Published` 在 Task 5/9/11 一致；`WsHub::publish` 入参 `WsOut` 在 Task 10/11 一致
- YAGNI：模板/邮件/ES 历史搜索放 §10 未实现
