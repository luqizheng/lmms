# Phase 1-A：libs 内核演进 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将现有 `libs/common` 拆分为职责单一的 9 个 crate，并补齐多数据库方言（mysql/dameng/mssql/sqlite）与 SQLite 单测夹具，为后续 contracts 与 iam-service 合并奠基。

**Architecture:** 演进而非破坏。保留 `libs/common` 作为兼容外观（re-export），增量新增独立 crate；逐 crate 写测、迁移、删旧。`common-db` 用 sqlx + Dialect trait + cargo feature 控制方言，业务代码方言无关。

**Tech Stack:** Rust 2021 / sqlx 0.8 / tokio / axum 0.8 / tracing 0.1 / thiserror 2 / Argon2 / Redis 0.27 / Moka / tempfile / aws_lc_rs。

参照设计：[2026-06-13-enterprise-base-libs-design.md](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-13-enterprise-base-libs-design.md) §3、§5。

执行边界：本计划仅触及 `libs/`、根 `Cargo.toml`、`migrations/_libs-fixture/`。**不**修改 `services/`、`sdk/`。
执行环境：所有 cargo 命令必须在真实 PowerShell（vcvarsall x64）中跑（项目规则）。

---

## 总体文件结构

- 新增 `libs/common-error/`
- 新增 `libs/common-log/`
- 新增 `libs/common-config/`
- 新增 `libs/common-db/`（含 dialect/ 与 test_kit/）
- 新增 `libs/common-cache/`
- 新增 `libs/common-http/`
- 新增 `libs/common-crypto/`
- 新增 `libs/common-validator/`
- 修改 `libs/common/`（改为兼容外观）
- 修改 `Cargo.toml` workspace.members 与 workspace.dependencies
- 新增 `migrations/_libs-fixture/sqlite/V001__libs_fixture.sql`
- 新增 `libs/README.md`

---

## Task 1：workspace 准备 + common-error

**Files:**
- Modify: `Cargo.toml`
- Create: `libs/common-error/Cargo.toml`
- Create: `libs/common-error/src/lib.rs`
- Test: `libs/common-error/tests/error_test.rs`

- [ ] **Step 1: 写失败测试**

`libs/common-error/tests/error_test.rs`：

```rust
use common_error::{AppError, ErrorKind};

#[test]
fn bad_request_has_correct_kind() {
    let e = AppError::bad_request("E_X", "bad");
    assert_eq!(e.kind(), ErrorKind::BadRequest);
    assert_eq!(e.code(), "E_X");
}

#[test]
fn from_sqlx_row_not_found_maps_to_not_found() {
    let sqlx_err = sqlx::Error::RowNotFound;
    let app: AppError = sqlx_err.into();
    assert_eq!(app.kind(), ErrorKind::NotFound);
}
```

- [ ] **Step 2: 注册 workspace 成员与公共依赖**

修改 `Cargo.toml`，在 `[workspace] members` 增加 `"libs/common-error"`，在 `[workspace.dependencies]` 增加：

```toml
moka = { version = "0.12", features = ["future"] }
tempfile = "3"
validator = { version = "0.18", features = ["derive"] }
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
aes-gcm = "0.10"
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls-aws-lc-rs"] }
```

> 注意：原 workspace 已有 `sqlx`（带 `mysql`），将其替换为以上不带 `mysql` 的版本，方言由各 crate 通过 features 自行启用。`libs/common` 在 Task 10 显式启用 `common-db/mysql` 维持兼容。

- [ ] **Step 3: 写 crate**

`libs/common-error/Cargo.toml`：

```toml
[package]
name = "common-error"
version = "0.1.0"
edition = "2021"

[dependencies]
thiserror.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx = { workspace = true }
reqwest.workspace = true
axum.workspace = true
```

`libs/common-error/src/lib.rs`：

```rust
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Internal,
    External,
}

#[derive(Debug, Error)]
#[error("[{code}] {message}")]
pub struct AppError {
    kind: ErrorKind,
    code: String,
    message: String,
    request_id: Option<String>,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl AppError {
    pub fn new(kind: ErrorKind, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { kind, code: code.into(), message: message.into(), request_id: None, source: None }
    }
    pub fn bad_request(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::BadRequest, code, msg) }
    pub fn unauthorized(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::Unauthorized, code, msg) }
    pub fn forbidden(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::Forbidden, code, msg) }
    pub fn not_found(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::NotFound, code, msg) }
    pub fn conflict(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::Conflict, code, msg) }
    pub fn internal(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::Internal, code, msg) }
    pub fn external(code: impl Into<String>, msg: impl Into<String>) -> Self { Self::new(ErrorKind::External, code, msg) }

    pub fn kind(&self) -> ErrorKind { self.kind }
    pub fn code(&self) -> &str { &self.code }
    pub fn message(&self) -> &str { &self.message }
    pub fn with_request_id(mut self, rid: impl Into<String>) -> Self { self.request_id = Some(rid.into()); self }
    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(mut self, e: E) -> Self {
        self.source = Some(Box::new(e)); self
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::RowNotFound => AppError::not_found("DB_NOT_FOUND", "row not found").with_source(e),
            _ => AppError::internal("DB_ERR", "database error").with_source(e),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::external("HTTP_ERR", e.to_string()).with_source(e)
    }
}

#[derive(Serialize)]
struct ErrBody<'a> { code: &'a str, message: &'a str, request_id: Option<&'a str> }

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.kind {
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::External => StatusCode::BAD_GATEWAY,
        };
        let body = ErrBody { code: &self.code, message: &self.message, request_id: self.request_id.as_deref() };
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
```

- [ ] **Step 4: 跑测试**

Run: `cargo test -p common-error`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add libs/common-error Cargo.toml
git commit -m "feat(libs): add common-error crate with AppError + axum IntoResponse"
```

---

## Task 2：common-log

**Files:**
- Create: `libs/common-log/Cargo.toml`
- Create: `libs/common-log/src/lib.rs`
- Create: `libs/common-log/src/request_id.rs`
- Create: `libs/common-log/src/middleware.rs`
- Test: `libs/common-log/tests/log_test.rs`
- Modify: `Cargo.toml` members += `"libs/common-log"`

- [ ] **Step 1: 写失败测试**

`libs/common-log/tests/log_test.rs`：

```rust
use common_log::request_id::RequestId;

#[test]
fn request_id_generates_uuid_when_absent() {
    let rid = RequestId::from_header(None);
    assert!(!rid.as_str().is_empty());
}

#[test]
fn request_id_passthrough_header() {
    let rid = RequestId::from_header(Some("abc-123"));
    assert_eq!(rid.as_str(), "abc-123");
}
```

- [ ] **Step 2: 写 crate**

`libs/common-log/Cargo.toml`：

```toml
[package]
name = "common-log"
version = "0.1.0"
edition = "2021"

[dependencies]
tracing.workspace = true
tracing-subscriber.workspace = true
uuid.workspace = true
common-error = { path = "../common-error" }
axum.workspace = true
tower.workspace = true
serde.workspace = true
serde_json.workspace = true
```

`libs/common-log/src/lib.rs`：

```rust
pub mod middleware;
pub mod request_id;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_json(default_level: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_target(true).with_current_span(true))
        .init();
}
```

`libs/common-log/src/request_id.rs`：

```rust
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RequestId(String);

impl RequestId {
    pub fn from_header(h: Option<&str>) -> Self {
        match h {
            Some(v) if !v.is_empty() => Self(v.to_string()),
            _ => Self(Uuid::new_v4().to_string()),
        }
    }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

`libs/common-log/src/middleware.rs`：

```rust
use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

use crate::request_id::RequestId;

pub async fn request_id_layer(mut req: Request, next: Next) -> Response {
    let incoming = req.headers().get("X-Request-Id").and_then(|v| v.to_str().ok());
    let rid = RequestId::from_header(incoming);
    req.extensions_mut().insert(rid.clone());
    let mut resp = next.run(req).await;
    if let Ok(v) = HeaderValue::from_str(rid.as_str()) {
        resp.headers_mut().insert("X-Request-Id", v);
    }
    resp
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p common-log`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add libs/common-log Cargo.toml
git commit -m "feat(libs): add common-log with request_id middleware + JSON tracing init"
```

---

## Task 3：common-config

**Files:**
- Create: `libs/common-config/Cargo.toml`
- Create: `libs/common-config/src/lib.rs`
- Test: `libs/common-config/tests/config_test.rs`
- Modify: `Cargo.toml` members += `"libs/common-config"`

- [ ] **Step 1: 写失败测试**

`libs/common-config/tests/config_test.rs`：

```rust
use common_config::Loader;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Demo { name: String, port: u16 }

#[test]
fn loader_reads_env_first() {
    std::env::set_var("DEMO_NAME", "envname");
    std::env::set_var("DEMO_PORT", "9999");
    let cfg: Demo = Loader::new("DEMO").load().unwrap();
    assert_eq!(cfg, Demo { name: "envname".into(), port: 9999 });
}
```

- [ ] **Step 2: 写 crate**

`libs/common-config/Cargo.toml`：

```toml
[package]
name = "common-config"
version = "0.1.0"
edition = "2021"

[dependencies]
config.workspace = true
serde.workspace = true
common-error = { path = "../common-error" }
```

`libs/common-config/src/lib.rs`：

```rust
use common_error::{AppError, AppResult};
use config::{Config, Environment};
use serde::de::DeserializeOwned;

pub struct Loader {
    prefix: String,
    file: Option<String>,
}

impl Loader {
    pub fn new(env_prefix: impl Into<String>) -> Self {
        Self { prefix: env_prefix.into(), file: None }
    }
    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.file = Some(path.into()); self
    }
    pub fn load<T: DeserializeOwned>(self) -> AppResult<T> {
        let mut b = Config::builder();
        if let Some(f) = self.file {
            b = b.add_source(config::File::with_name(&f).required(false));
        }
        b = b.add_source(Environment::with_prefix(&self.prefix).separator("_"));
        let c = b.build().map_err(|e| AppError::internal("CFG_BUILD", e.to_string()))?;
        c.try_deserialize::<T>().map_err(|e| AppError::internal("CFG_PARSE", e.to_string()))
    }
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p common-config`
Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add libs/common-config Cargo.toml
git commit -m "feat(libs): add common-config Loader (env+file)"
```

---

## Task 4：common-db Dialect 骨架

**Files:**
- Create: `libs/common-db/Cargo.toml`
- Create: `libs/common-db/src/lib.rs`
- Create: `libs/common-db/src/dialect/mod.rs`
- Create: `libs/common-db/src/dialect/sqlite.rs`
- Create: `libs/common-db/src/dialect/mysql.rs`
- Create: `libs/common-db/src/dialect/dameng.rs`
- Create: `libs/common-db/src/dialect/mssql.rs`
- Test: `libs/common-db/tests/dialect_test.rs`
- Modify: `Cargo.toml` members += `"libs/common-db"`

- [ ] **Step 1: 写失败测试**

`libs/common-db/tests/dialect_test.rs`：

```rust
use common_db::dialect::{DialectKind, Paginate};

#[test]
fn sqlite_pagination_uses_limit_offset() {
    assert_eq!(DialectKind::Sqlite.paginate(20, 40), "LIMIT 20 OFFSET 40");
}

#[test]
fn mssql_pagination_uses_offset_fetch() {
    assert_eq!(DialectKind::Mssql.paginate(20, 40), "OFFSET 40 ROWS FETCH NEXT 20 ROWS ONLY");
}

#[test]
fn detect_from_url_picks_sqlite() {
    assert_eq!(DialectKind::from_url("sqlite::memory:"), DialectKind::Sqlite);
}

#[test]
fn detect_from_url_picks_dameng() {
    assert_eq!(DialectKind::from_url("dameng://user:pwd@host:5236/db"), DialectKind::Dameng);
}
```

- [ ] **Step 2: 写 crate**

`libs/common-db/Cargo.toml`：

```toml
[package]
name = "common-db"
version = "0.1.0"
edition = "2021"

[features]
default = ["sqlite"]
mysql = ["sqlx/mysql"]
dameng = ["sqlx/mysql"]
mssql = ["sqlx/mssql"]
sqlite = ["sqlx/sqlite"]
test-kit = ["sqlite", "dep:tempfile"]

[dependencies]
sqlx = { workspace = true, features = ["macros", "chrono"] }
tokio = { workspace = true }
chrono = { workspace = true }
common-error = { path = "../common-error" }
async-trait.workspace = true
tempfile = { workspace = true, optional = true }

[dev-dependencies]
tempfile = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

`libs/common-db/src/lib.rs`：

```rust
pub mod dialect;
#[cfg(feature = "test-kit")]
pub mod test_kit;
```

`libs/common-db/src/dialect/mod.rs`：

```rust
pub mod sqlite;
pub mod mysql;
pub mod dameng;
pub mod mssql;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialectKind {
    Mysql,
    Dameng,
    Mssql,
    Sqlite,
}

pub trait Paginate {
    fn paginate(&self, limit: u32, offset: u32) -> String;
}

impl Paginate for DialectKind {
    fn paginate(&self, limit: u32, offset: u32) -> String {
        match self {
            DialectKind::Mysql | DialectKind::Dameng | DialectKind::Sqlite =>
                format!("LIMIT {} OFFSET {}", limit, offset),
            DialectKind::Mssql =>
                format!("OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", offset, limit),
        }
    }
}

impl DialectKind {
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("sqlite:") { Self::Sqlite }
        else if url.starts_with("dameng:") { Self::Dameng }
        else if url.starts_with("mssql:") || url.starts_with("sqlserver:") { Self::Mssql }
        else { Self::Mysql }
    }
    pub fn now_expr(&self) -> &'static str {
        match self {
            Self::Mssql => "SYSUTCDATETIME()",
            Self::Sqlite => "CURRENT_TIMESTAMP",
            _ => "UTC_TIMESTAMP()",
        }
    }
}
```

`libs/common-db/src/dialect/sqlite.rs`、`mysql.rs`、`dameng.rs`、`mssql.rs` 每个文件内容相同：

```rust
// reserved for future dialect-specific helpers
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p common-db --features sqlite`
Expected: 4 passed.

- [ ] **Step 4: Commit**

```bash
git add libs/common-db Cargo.toml
git commit -m "feat(libs): add common-db with DialectKind + paginate + url detection"
```

---

## Task 5：common-db SQLite 测试夹具

**Files:**
- Create: `libs/common-db/src/test_kit/mod.rs`
- Create: `libs/common-db/src/test_kit/sqlite_fixture.rs`
- Create: `migrations/_libs-fixture/sqlite/V001__libs_fixture.sql`
- Test: `libs/common-db/tests/sqlite_fixture_test.rs`

- [ ] **Step 1: 写夹具 SQL**

`migrations/_libs-fixture/sqlite/V001__libs_fixture.sql`：

```sql
CREATE TABLE IF NOT EXISTS demo_user (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

- [ ] **Step 2: 写失败测试**

`libs/common-db/tests/sqlite_fixture_test.rs`：

```rust
#![cfg(feature = "test-kit")]
use common_db::test_kit::SqliteFixture;
use sqlx::Row;

#[tokio::test]
async fn fixture_isolates_per_test() {
    let fx = SqliteFixture::new("migrations/_libs-fixture/sqlite").await.unwrap();
    sqlx::query("INSERT INTO demo_user(name) VALUES('alice')")
        .execute(fx.pool()).await.unwrap();
    let row = sqlx::query("SELECT COUNT(*) AS c FROM demo_user")
        .fetch_one(fx.pool()).await.unwrap();
    let c: i64 = row.get("c");
    assert_eq!(c, 1);
}

#[tokio::test]
async fn fixture_parallel_does_not_collide() {
    let a = SqliteFixture::new("migrations/_libs-fixture/sqlite").await.unwrap();
    let b = SqliteFixture::new("migrations/_libs-fixture/sqlite").await.unwrap();
    sqlx::query("INSERT INTO demo_user(name) VALUES('x')")
        .execute(a.pool()).await.unwrap();
    let row = sqlx::query("SELECT COUNT(*) AS c FROM demo_user")
        .fetch_one(b.pool()).await.unwrap();
    let c: i64 = row.get("c");
    assert_eq!(c, 0, "fixtures must not share state");
}
```

- [ ] **Step 3: 写 test_kit 实现**

`libs/common-db/src/test_kit/mod.rs`：

```rust
pub mod sqlite_fixture;
pub use sqlite_fixture::SqliteFixture;
```

`libs/common-db/src/test_kit/sqlite_fixture.rs`：

```rust
use common_error::{AppError, AppResult};
use sqlx::SqlitePool;
use std::path::Path;
use tempfile::NamedTempFile;

pub struct SqliteFixture {
    _file: NamedTempFile,
    pool: SqlitePool,
    pub path: String,
}

impl SqliteFixture {
    pub async fn new(migration_dir: &str) -> AppResult<Self> {
        let file = NamedTempFile::new()
            .map_err(|e| AppError::internal("FX_TMP", e.to_string()))?;
        let path = file.path().to_string_lossy().to_string();
        let url = format!("sqlite://{}?mode=rwc", path);
        let pool = SqlitePool::connect(&url).await
            .map_err(|e| AppError::internal("FX_CONN", e.to_string()))?;
        Self::run_migrations(&pool, migration_dir).await?;
        Ok(Self { _file: file, pool, path })
    }

    pub fn pool(&self) -> &SqlitePool { &self.pool }

    async fn run_migrations(pool: &SqlitePool, dir: &str) -> AppResult<()> {
        let p = Path::new(dir);
        let mut entries: Vec<_> = std::fs::read_dir(p)
            .map_err(|e| AppError::internal("FX_DIR", format!("{}: {}", dir, e)))?
            .filter_map(|r| r.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if e.path().extension().and_then(|s| s.to_str()) == Some("sql") {
                let sql = std::fs::read_to_string(e.path())
                    .map_err(|err| AppError::internal("FX_READ", err.to_string()))?;
                sqlx::raw_sql(&sql).execute(pool).await
                    .map_err(|err| AppError::internal("FX_EXEC", err.to_string()))?;
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: 跑测试**

Run: `cargo test -p common-db --features test-kit`
Expected: 6 passed（含 Task 4 的 4 个 + 此 2 个）。

- [ ] **Step 5: Commit**

```bash
git add libs/common-db migrations/_libs-fixture
git commit -m "feat(libs): add common-db SqliteFixture for parallel-safe unit tests"
```

---

## Task 6：common-cache（Memory + trait）

**Files:**
- Create: `libs/common-cache/Cargo.toml`
- Create: `libs/common-cache/src/lib.rs`
- Create: `libs/common-cache/src/memory.rs`
- Test: `libs/common-cache/tests/cache_test.rs`
- Modify: `Cargo.toml` members += `"libs/common-cache"`

- [ ] **Step 1: 写失败测试**

`libs/common-cache/tests/cache_test.rs`：

```rust
use common_cache::{Cache, MemoryCache};
use std::time::Duration;

#[tokio::test]
async fn memory_set_get_roundtrip() {
    let c = MemoryCache::new(64);
    c.set("k", b"v".to_vec(), Some(Duration::from_secs(60))).await.unwrap();
    let got = c.get("k").await.unwrap();
    assert_eq!(got.as_deref(), Some(&b"v"[..]));
}

#[tokio::test]
async fn memory_del_removes_key() {
    let c = MemoryCache::new(64);
    c.set("k", b"v".to_vec(), None).await.unwrap();
    c.del("k").await.unwrap();
    assert!(c.get("k").await.unwrap().is_none());
}
```

- [ ] **Step 2: 写 crate**

`libs/common-cache/Cargo.toml`：

```toml
[package]
name = "common-cache"
version = "0.1.0"
edition = "2021"

[features]
default = ["memory"]
memory = []
redis = ["dep:redis"]

[dependencies]
async-trait.workspace = true
moka = { workspace = true }
common-error = { path = "../common-error" }
redis = { workspace = true, optional = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

`libs/common-cache/src/lib.rs`：

```rust
pub mod memory;
pub use memory::MemoryCache;

use async_trait::async_trait;
use common_error::AppResult;
use std::time::Duration;

#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>>;
    async fn set(&self, key: &str, val: Vec<u8>, ttl: Option<Duration>) -> AppResult<()>;
    async fn del(&self, key: &str) -> AppResult<()>;
}
```

`libs/common-cache/src/memory.rs`：

```rust
use crate::Cache;
use async_trait::async_trait;
use common_error::AppResult;
use moka::future::Cache as Moka;
use std::time::Duration;

pub struct MemoryCache {
    inner: Moka<String, Vec<u8>>,
}

impl MemoryCache {
    pub fn new(capacity: u64) -> Self {
        Self { inner: Moka::builder().max_capacity(capacity).build() }
    }
}

#[async_trait]
impl Cache for MemoryCache {
    async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        Ok(self.inner.get(key).await)
    }
    async fn set(&self, key: &str, val: Vec<u8>, _ttl: Option<Duration>) -> AppResult<()> {
        self.inner.insert(key.to_string(), val).await; Ok(())
    }
    async fn del(&self, key: &str) -> AppResult<()> {
        self.inner.invalidate(key).await; Ok(())
    }
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p common-cache`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add libs/common-cache Cargo.toml
git commit -m "feat(libs): add common-cache Cache trait + MemoryCache(moka)"
```

---

## Task 7：common-http

**Files:**
- Create: `libs/common-http/Cargo.toml`
- Create: `libs/common-http/src/lib.rs`
- Test: `libs/common-http/tests/http_test.rs`
- Modify: `Cargo.toml` members += `"libs/common-http"`

- [ ] **Step 1: 写失败测试**

`libs/common-http/tests/http_test.rs`：

```rust
use common_http::HttpClient;

#[tokio::test]
async fn client_builds_with_request_id_header() {
    let c = HttpClient::builder().request_id("abc-1").build().unwrap();
    assert_eq!(c.default_request_id(), Some("abc-1"));
}
```

- [ ] **Step 2: 写 crate**

`libs/common-http/Cargo.toml`：

```toml
[package]
name = "common-http"
version = "0.1.0"
edition = "2021"

[dependencies]
reqwest.workspace = true
tokio = { workspace = true }
common-error = { path = "../common-error" }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

`libs/common-http/src/lib.rs`：

```rust
use common_error::{AppError, AppResult};
use reqwest::Client;
use std::time::Duration;

pub struct HttpClient {
    inner: Client,
    request_id: Option<String>,
}

pub struct HttpClientBuilder {
    timeout: Duration,
    request_id: Option<String>,
}

impl HttpClient {
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder { timeout: Duration::from_secs(10), request_id: None }
    }
    pub fn inner(&self) -> &Client { &self.inner }
    pub fn default_request_id(&self) -> Option<&str> { self.request_id.as_deref() }
}

impl HttpClientBuilder {
    pub fn timeout(mut self, t: Duration) -> Self { self.timeout = t; self }
    pub fn request_id(mut self, rid: impl Into<String>) -> Self { self.request_id = Some(rid.into()); self }
    pub fn build(self) -> AppResult<HttpClient> {
        let inner = Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| AppError::internal("HTTP_BUILD", e.to_string()))?;
        Ok(HttpClient { inner, request_id: self.request_id })
    }
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p common-http`
Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add libs/common-http Cargo.toml
git commit -m "feat(libs): add common-http builder skeleton"
```

---

## Task 8：common-crypto（Argon2 + AES-GCM）

**Files:**
- Create: `libs/common-crypto/Cargo.toml`
- Create: `libs/common-crypto/src/lib.rs`
- Create: `libs/common-crypto/src/password.rs`
- Create: `libs/common-crypto/src/aead.rs`
- Test: `libs/common-crypto/tests/crypto_test.rs`
- Modify: `Cargo.toml` members += `"libs/common-crypto"`（aes-gcm 已在 Task 1 加入 workspace.dependencies）

- [ ] **Step 1: 写失败测试**

`libs/common-crypto/tests/crypto_test.rs`：

```rust
use common_crypto::password::{hash, verify};
use common_crypto::aead::{Aead, gen_key};

#[test]
fn password_roundtrip() {
    let h = hash("secret").unwrap();
    assert!(verify("secret", &h).unwrap());
    assert!(!verify("wrong", &h).unwrap());
}

#[test]
fn aead_seal_open_roundtrip() {
    let key = gen_key();
    let aead = Aead::new(&key);
    let ct = aead.seal(b"hello", b"aad").unwrap();
    let pt = aead.open(&ct, b"aad").unwrap();
    assert_eq!(pt, b"hello");
}
```

- [ ] **Step 2: 写 crate**

`libs/common-crypto/Cargo.toml`：

```toml
[package]
name = "common-crypto"
version = "0.1.0"
edition = "2021"

[dependencies]
argon2.workspace = true
rand.workspace = true
aes-gcm = { workspace = true }
common-error = { path = "../common-error" }
```

`libs/common-crypto/src/lib.rs`：

```rust
pub mod password;
pub mod aead;
```

`libs/common-crypto/src/password.rs`：

```rust
use argon2::{password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString}, Argon2};
use common_error::{AppError, AppResult};

pub fn hash(plain: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let h = Argon2::default().hash_password(plain.as_bytes(), &salt)
        .map_err(|e| AppError::internal("PWD_HASH", e.to_string()))?;
    Ok(h.to_string())
}

pub fn verify(plain: &str, hash_str: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(hash_str)
        .map_err(|e| AppError::internal("PWD_PARSE", e.to_string()))?;
    Ok(Argon2::default().verify_password(plain.as_bytes(), &parsed).is_ok())
}
```

`libs/common-crypto/src/aead.rs`：

```rust
use aes_gcm::{aead::{Aead as _, AeadCore, KeyInit, OsRng}, Aes256Gcm, Key, Nonce};
use common_error::{AppError, AppResult};

pub fn gen_key() -> [u8; 32] {
    use rand::RngCore;
    let mut k = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut k);
    k
}

pub struct Aead { cipher: Aes256Gcm }

impl Aead {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key)) }
    }
    pub fn seal(&self, plaintext: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = self.cipher
            .encrypt(&nonce, aes_gcm::aead::Payload { msg: plaintext, aad })
            .map_err(|e| AppError::internal("AEAD_SEAL", e.to_string()))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ct);
        Ok(out)
    }
    pub fn open(&self, blob: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
        if blob.len() < 12 { return Err(AppError::bad_request("AEAD_LEN", "blob too short")); }
        let (n, c) = blob.split_at(12);
        let nonce = Nonce::from_slice(n);
        self.cipher
            .decrypt(nonce, aes_gcm::aead::Payload { msg: c, aad })
            .map_err(|e| AppError::internal("AEAD_OPEN", e.to_string()))
    }
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p common-crypto`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add libs/common-crypto Cargo.toml
git commit -m "feat(libs): add common-crypto Argon2 + AES-256-GCM"
```

---

## Task 9：common-validator

**Files:**
- Create: `libs/common-validator/Cargo.toml`
- Create: `libs/common-validator/src/lib.rs`
- Test: `libs/common-validator/tests/validator_test.rs`
- Modify: `Cargo.toml` members += `"libs/common-validator"`

- [ ] **Step 1: 写失败测试**

`libs/common-validator/tests/validator_test.rs`：

```rust
use common_validator::{is_cn_mobile, is_strong_password};

#[test]
fn cn_mobile_accepts_valid() {
    assert!(is_cn_mobile("13912345678"));
    assert!(!is_cn_mobile("12012345678"));
    assert!(!is_cn_mobile("1391234567"));
}

#[test]
fn strong_password_requires_classes_and_length() {
    assert!(is_strong_password("Aa1!aaaa"));
    assert!(!is_strong_password("aaaaaaaa"));
    assert!(!is_strong_password("Aa1!aa"));
}
```

- [ ] **Step 2: 写 crate**

`libs/common-validator/Cargo.toml`：

```toml
[package]
name = "common-validator"
version = "0.1.0"
edition = "2021"

[dependencies]
validator.workspace = true
common-error = { path = "../common-error" }
```

`libs/common-validator/src/lib.rs`：

```rust
pub fn is_cn_mobile(s: &str) -> bool {
    if s.len() != 11 { return false; }
    if !s.chars().all(|c| c.is_ascii_digit()) { return false; }
    let b = s.as_bytes();
    b[0] == b'1' && matches!(b[1], b'3'..=b'9')
}

pub fn is_strong_password(s: &str) -> bool {
    if s.len() < 8 { return false; }
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let has_sym   = s.chars().any(|c| !c.is_ascii_alphanumeric());
    [has_lower, has_upper, has_digit, has_sym].iter().filter(|x| **x).count() >= 3
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p common-validator`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add libs/common-validator Cargo.toml
git commit -m "feat(libs): add common-validator (cn_mobile + strong_password)"
```

---

## Task 10：libs/common 改为兼容外观

**Files:**
- Modify: `libs/common/Cargo.toml`
- Modify: `libs/common/src/lib.rs`
- Modify: `libs/common/src/error.rs`
- Modify: `libs/common/src/db.rs`
- Modify: `libs/common/src/password.rs`

目标：保持现有 `services/*` 与 `sdk/*` 编译通过；老路径（`common::error::CommonError`、`common::db::connect`、`common::password::*`）继续可用，内部转发到新 crate。

- [ ] **Step 1: 改 Cargo.toml**

修改 `libs/common/Cargo.toml`：

```toml
[package]
name = "common"
version = "0.1.0"
edition = "2021"

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
argon2.workspace = true
anyhow = "1"
sqlx = { workspace = true, features = ["mysql"] }
redis.workspace = true
reqwest.workspace = true
tokio = { workspace = true, features = ["sync"] }

common-error = { path = "../common-error" }
common-db = { path = "../common-db", features = ["mysql"] }
common-crypto = { path = "../common-crypto" }
```

- [ ] **Step 2: 改 lib.rs（保持原模块）**

`libs/common/src/lib.rs` 不变（仍 `pub mod error/token/config/password/db/redisx/http`）。

- [ ] **Step 3: 改 error.rs**

`libs/common/src/error.rs`：

```rust
pub use common_error::{AppError, AppResult, ErrorKind};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommonError {
    #[error("JWT error: {0}")]
    JwtError(String),
    #[error("Invalid token claims")]
    InvalidClaims,
    #[error("Token expired")]
    TokenExpired,
    #[error("Token revoked")]
    TokenRevoked,
    #[error("Unknown error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, CommonError>;
```

- [ ] **Step 4: 改 db.rs**

`libs/common/src/db.rs`：

```rust
pub use common_db::dialect::DialectKind;

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

pub async fn connect(url: &str, max: u32) -> anyhow::Result<MySqlPool> {
    Ok(MySqlPoolOptions::new()
        .max_connections(max)
        .acquire_timeout(Duration::from_secs(5))
        .connect(url).await?)
}
```

- [ ] **Step 5: 改 password.rs**

读取当前实现：保留对外公开的所有函数与签名，把内部计算切换到 `common_crypto::password`。例：

```rust
use anyhow::Result;

pub fn hash_password(plain: &str) -> Result<String> {
    common_crypto::password::hash(plain).map_err(|e| anyhow::anyhow!(e.to_string()))
}

pub fn verify_password(plain: &str, hash: &str) -> Result<bool> {
    common_crypto::password::verify(plain, hash).map_err(|e| anyhow::anyhow!(e.to_string()))
}
```

> 若现有 `password.rs` 有其他公开函数，逐一保留签名，内部仅切实现。**禁止重命名公开函数**。

- [ ] **Step 6: 跑全 workspace**

Run: `cargo build --workspace`
Expected: 成功（含 services/* 与 sdk/*）。

Run: `cargo test --workspace --exclude common-db`
Expected: 所有现有测试通过；`libs/common/tests/password_test.rs` 仍通过。

Run: `cargo test -p common-db --features test-kit`
Expected: 6 passed。

- [ ] **Step 7: Commit**

```bash
git add libs/common
git commit -m "refactor(libs): make libs/common a thin facade over new common-* crates"
```

---

## Task 11：libs/README + workspace 收尾

**Files:**
- Create: `libs/README.md`
- Modify: `Cargo.toml`（确认 members 全量）

- [ ] **Step 1: 写 libs/README.md**

```markdown
# libs/ 共享内核

| crate | 职责 | 关键依赖 |
|---|---|---|
| common-error | AppError + axum IntoResponse | thiserror / axum |
| common-log | tracing JSON + request_id 中间件 | tracing-subscriber |
| common-config | 配置加载（env + file） | config |
| common-db | sqlx 多方言（mysql/dameng/mssql/sqlite）+ SqliteFixture | sqlx + tempfile |
| common-cache | Cache trait + MemoryCache(Moka) / Redis(可选) | moka / redis |
| common-http | reqwest 封装 + 请求 ID 透传 | reqwest |
| common-crypto | Argon2 + AES-256-GCM | argon2 / aes-gcm |
| common-validator | 中国手机号 / 强密码等 | validator |
| common（外观） | 兼容旧路径，转发到新 crate | — |
| jwt-ctx / authz | JWT 上下文 + RBAC | (已存在) |

## feature 矩阵

- `common-db`：`mysql` / `dameng` / `mssql` / `sqlite` / `test-kit`，互不强制。
- `common-cache`：`memory`（默认）/ `redis`。

## 单测约定

- 用 `common-db` 的 `SqliteFixture` 做 DB 单测；每用例独立 tempfile，并行安全。
- 全部 cargo 命令在真实 PowerShell（vcvarsall x64）跑。
```

- [ ] **Step 2: workspace 全量校验**

Run: `cargo fmt --check`
Expected: pass。

Run: `cargo clippy --all-targets -- -D warnings`
Expected: pass。

- [ ] **Step 3: Commit**

```bash
git add libs/README.md
git commit -m "docs(libs): add libs README with crate matrix and feature flags"
```

---

## 自查（plan self-review）

1. Spec 覆盖：
   - §3 多方言 → Task 4/5
   - §5.1 common-error → Task 1
   - §5.2 common-config → Task 3
   - §5.3 common-log → Task 2
   - §5.4 common-http → Task 7（骨架；重试/熔断/服务发现留 Phase 3）
   - §5.5 common-db → Task 4/5
   - §5.6 common-cache → Task 6（Memory；Redis 留 Phase 3）
   - §5.7 common-crypto → Task 8（Argon2 + AES-GCM；RSA/HMAC/KMS 留 Phase 3）
   - §5.8 common-validator → Task 9
   - §5.9 jwt-ctx / authz → 已存在，未触及
2. 兼容性：Task 10 让 `libs/common` 转发，services/sdk 编译不破。
3. 占位符：无 TBD。
4. 类型一致性：`AppError` / `AppResult` / `DialectKind` / `Cache` 跨任务一致。
5. 范围控制：本计划仅做 Phase 1-A（libs 内核）。contracts 与 iam-service 合并不在本计划。

---

## 执行选择

**Plan complete and saved.** 两种执行方式：

1. **Subagent-Driven（推荐）**——每个 Task 派出新 subagent，逐 Task review。
2. **Inline Execution**——本会话 executing-plans，分段执行加 checkpoint。

请告知选择哪种。