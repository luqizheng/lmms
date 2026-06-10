# User Service 多数据库支持 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `services/user-service` 增加 MySQL / MSSQL / SQLite / 达梦 四种数据库的连接层支持；通过 `database.db_type` 配置切换；非 MySQL 路径在 service 层返回 501。

**Architecture:** `DbType` 枚举（serde 解析）→ `init_pool` 按 db_type 分发 → 三种 sqlx 后端 + 一种 odbc-api 达梦 → 统一封装为 `DbPool` 枚举 → service / routes 层签名改 `&DbPool`，函数体加 `match`；非 MySQL 分支返回 `AppError::UnsupportedDb`。

**Tech Stack:** Rust 1.x, axum 0.7, sqlx 0.8 (`mysql` + `mssql` + `sqlite`), odbc-api 8.x, encoding_rs 0.8, thiserror.

**Spec:** [`docs/superpowers/specs/2026-06-10-user-service-multi-db-design.md`](../specs/2026-06-10-user-service-multi-db-design.md)

**前置条件**:
- 真实 PowerShell（vcvarsall x64）环境跑 cargo，沙箱内只允许只读命令
- 达梦相关代码改动不要求本机装 DM8 ODBC 驱动（连接测试需装）

**Commit 前缀**: `feat(user-service):` / `fix(user-service):`

---

## File Structure

**Modify:**
```
services/user-service/Cargo.toml                                    # 加 mssql/sqlite features + odbc-api + encoding_rs
services/user-service/src/config.rs                                 # 加 DbType 枚举 + database.db_type 字段
services/user-service/src/db.rs                                     # 重写为 DbPool 枚举 + init_pool
services/user-service/src/error.rs                                  # 加 UnsupportedDb 变体 + 501 映射
services/user-service/src/main.rs                                   # 调新 init_pool
services/user-service/src/service/user_svc.rs                       # &MySqlPool → &DbPool
services/user-service/src/service/role_svc.rs                       # 同上
services/user-service/src/service/claim_svc.rs                      # 同上
services/user-service/src/routes/health.rs                          # match 四种池
services/user-service/src/routes/users.rs                           # &MySqlPool → &DbPool
services/user-service/src/routes/internal.rs                        # 同上
services/user-service/src/routes/permissions.rs                     # 同上
services/user-service/src/routes/roles.rs                           # 同上
services/user-service/src/routes/user_roles.rs                      # 同上
services/user-service/config/config.yaml                            # 增 db_type 字段
services/user-service/tests/integration_test.rs                     # 末尾增 mod multi_db
```

---

## Task 1: 升级 Cargo.toml 依赖

**Files:**
- Modify: `services/user-service/Cargo.toml`

- [ ] **Step 1: 修改 sqlx features 行**

`services/user-service/Cargo.toml` 第 12 行改为:
```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "mysql", "mssql", "sqlite"] }
```

- [ ] **Step 2: 追加 odbc-api 和 encoding_rs**

在 `[dependencies]` 末尾追加:
```toml
odbc-api = { version = "8.1", features = ["derive"] }
encoding_rs = "0.8"
```

- [ ] **Step 3: 提交**

```bash
git add services/user-service/Cargo.toml
git commit -m "feat(user-service): add mssql, sqlite, odbc-api deps for multi-db"
```

---

## Task 2: 添加 DbType 枚举和 config 字段

**Files:**
- Modify: `services/user-service/src/config.rs`

- [ ] **Step 1: 在 `ServerCfg` 之前添加 DbType 枚举**

`services/user-service/src/config.rs`，在 `use` 语句之后、第一个 `#[derive]` 之前添加:
```rust
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Mysql,
    Mssql,
    Sqlite,
    Dameng,
}
```

- [ ] **Step 2: 给 DatabaseCfg 加 `db_type` 字段**

把:
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseCfg {
    pub url: String,
    pub max_connections: u32,
}
```

改为:
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseCfg {
    pub db_type: DbType,
    pub url: String,
    pub max_connections: u32,
}
```

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/config.rs
git commit -m "feat(user-service): add DbType enum and database.db_type field"
```

---

## Task 3: DbType 解析单元测试

**Files:**
- Modify: `services/user-service/src/config.rs`

- [ ] **Step 1: 在 config.rs 末尾追加测试模块**

在 `pub fn load_config` 之后追加:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_db_type_mysql() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Mysql);
    }

    #[test]
    fn parse_db_type_mssql() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: mssql
  url: "mssql://u:p@localhost:1433/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Mssql);
    }

    #[test]
    fn parse_db_type_sqlite() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: sqlite
  url: "sqlite://./test.db"
  max_connections: 1
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Sqlite);
    }

    #[test]
    fn parse_db_type_dameng() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: dameng
  url: "Driver={DM8 ODBC DRIVER};Server=127.0.0.1;Port=5236;UID=SYSDBA;PWD=x;Schema=DMHR"
  max_connections: 1
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Dameng);
    }

    #[test]
    fn parse_db_type_missing() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        assert!(serde_yaml::from_str::<Config>(yaml).is_err());
    }

    #[test]
    fn parse_db_type_invalid() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: oracle
  url: "x"
  max_connections: 1
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        assert!(serde_yaml::from_str::<Config>(yaml).is_err());
    }
}
```

- [ ] **Step 2: 在真实 PowerShell 跑测试**

```bash
cargo test -p user-service --lib config::tests
```

Expected: 6 个测试全 PASS。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/config.rs
git commit -m "test(user-service): add DbType serde parsing tests"
```

---

## Task 4: 重写 db.rs 为 DbPool 枚举

**Files:**
- Modify: `services/user-service/src/db.rs`

- [ ] **Step 1: 替换 db.rs 全部内容**

`services/user-service/src/db.rs` 整文件替换为:
```rust
use anyhow::{Context, Result};
use sqlx::mssql::MssqlPool;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::MssqlPoolOptions;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::DatabaseCfg;
use crate::config::DbType;

#[derive(Clone)]
pub enum DbPool {
    Mysql(MySqlPool),
    Mssql(MssqlPool),
    Sqlite(SqlitePool),
    Dameng(Arc<Mutex<odbc_api::Connection<'static>>>),
}

impl DbPool {
    pub fn db_type(&self) -> DbType {
        match self {
            Self::Mysql(_) => DbType::Mysql,
            Self::Mssql(_) => DbType::Mssql,
            Self::Sqlite(_) => DbType::Sqlite,
            Self::Dameng(_) => DbType::Dameng,
        }
    }
}

pub async fn init_pool(cfg: &DatabaseCfg) -> Result<DbPool> {
    match cfg.db_type {
        DbType::Mysql => {
            let pool = MySqlPoolOptions::new()
                .max_connections(cfg.max_connections)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&cfg.url)
                .await
                .with_context(|| format!("connect mysql {}", cfg.url))?;
            Ok(DbPool::Mysql(pool))
        }
        DbType::Mssql => {
            let pool = MssqlPoolOptions::new()
                .max_connections(cfg.max_connections)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&cfg.url)
                .await
                .with_context(|| format!("connect mssql {}", cfg.url))?;
            Ok(DbPool::Mssql(pool))
        }
        DbType::Sqlite => {
            let pool = SqlitePoolOptions::new()
                .max_connections(cfg.max_connections.max(1))
                .acquire_timeout(Duration::from_secs(5))
                .connect(&cfg.url)
                .await
                .with_context(|| format!("connect sqlite {}", cfg.url))?;
            Ok(DbPool::Sqlite(pool))
        }
        DbType::Dameng => {
            use odbc_api::{ConnectionOptions, Environment};
            let env = Environment::new().context("create odbc env")?;
            let conn = env
                .connect_with_connection_string(&cfg.url, ConnectionOptions::default())
                .context("connect dameng via odbc")?;
            Ok(DbPool::Dameng(Arc::new(Mutex::new(conn))))
        }
    }
}
```

- [ ] **Step 2: 在真实 PowerShell 检查编译**

```bash
cargo check -p user-service
```

Expected: 错误在 `main.rs` / service / routes（签名不匹配），db.rs 本身不报。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/db.rs
git commit -m "feat(user-service): add DbPool enum with mysql/mssql/sqlite/dameng"
```

---

## Task 5: 添加 UnsupportedDb 错误变体

**Files:**
- Modify: `services/user-service/src/error.rs`

- [ ] **Step 1: 读 error.rs 现状**

```bash
Read: services/user-service/src/error.rs
```

确认现有 `AppError` 变体和 `IntoResponse` 实现位置。

- [ ] **Step 2: 添加 DbType import 和 UnsupportedDb 变体**

在文件顶部 `use` 区域添加:
```rust
use crate::config::DbType;
```

在 `AppError` 枚举中追加:
```rust
#[error("db type {0:?} not supported for this operation")]
UnsupportedDb(DbType),
```

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/error.rs
git commit -m "feat(user-service): add UnsupportedDb error variant"
```

---

## Task 6: UnsupportedDb → 501 映射

**Files:**
- Modify: `services/user-service/src/error.rs`

- [ ] **Step 1: 在 IntoResponse match 中加分支**

在 `IntoResponse for AppError` 的 match 块中追加:
```rust
AppError::UnsupportedDb(t) => (
    StatusCode::NOT_IMPLEMENTED,
    Json(json!({"error": "unsupported_db", "db_type": format!("{:?}", t)})),
)
    .into_response(),
```

确认顶部已有:
```rust
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
```
缺哪个补哪个。

- [ ] **Step 2: 在文件末尾追加单元测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_db_status_is_501() {
        let resp = AppError::UnsupportedDb(DbType::Dameng).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
```

- [ ] **Step 3: 在真实 PowerShell 跑测试**

```bash
cargo test -p user-service --lib error::tests
```

Expected: PASS。

- [ ] **Step 4: 提交**

```bash
git add services/user-service/src/error.rs
git commit -m "feat(user-service): map UnsupportedDb to 501 with json body"
```

---

## Task 7: 更新 main.rs 调新 init_pool

**Files:**
- Modify: `services/user-service/src/main.rs`

- [ ] **Step 1: 替换 init_pool 调用**

`main.rs` 当前:
```rust
let pool = init_pool(&cfg.database.url, cfg.database.max_connections).await?;
```

改为:
```rust
let pool = init_pool(&cfg.database).await?;
tracing::info!(db_type = ?cfg.database.db_type, "user-service pool ready");
```

- [ ] **Step 2: 在真实 PowerShell 检查编译（应还有 service / routes 错误）**

```bash
cargo check -p user-service
```

Expected: error 数量比 Task 4 少（仅 service / routes）。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/main.rs
git commit -m "feat(user-service): wire new init_pool in main"
```

---

## Task 8: 改 service 层 user_svc.rs 签名

**Files:**
- Modify: `services/user-service/src/service/user_svc.rs`

- [ ] **Step 1: 改 use 行**

`use sqlx::MySqlPool;` 改为 `use crate::db::DbPool;`

- [ ] **Step 2: 改 `fetch_user_with_roles` 签名 + body**

签名: `pool: &MySqlPool` → `pool: &DbPool`

函数体外层包:
```rust
match pool {
    DbPool::Mysql(p) => {
        // 原 body
    }
    _ => Err(AppError::UnsupportedDb(pool.db_type())),
}
```

- [ ] **Step 3: 同样改 `fetch_user_by_code`、`list_users`、`update_user_extends`**

每个函数签名 `&MySqlPool` → `&DbPool`，函数体外包 match（MySQL 分支保留原 body，其他分支 UnsupportedDb）。

- [ ] **Step 4: 改 `fetch_user_role_codes`**

签名 `&MySqlPool` → `&DbPool`，包 match。

- [ ] **Step 5: 改 `assign_user_roles` / `remove_user_roles`**

同模式。`assign_user_roles` 中的 `pool.begin()` 需在 MySQL 分支内改为 `p.begin()`。

- [ ] **Step 6: 提交**

```bash
git add services/user-service/src/service/user_svc.rs
git commit -m "refactor(user-service): user_svc accepts DbPool, mysql branch only"
```

---

## Task 9: 改 service 层 role_svc.rs 签名

**Files:**
- Modify: `services/user-service/src/service/role_svc.rs`

- [ ] **Step 1: 读 role_svc.rs 全部函数**

```bash
Read: services/user-service/src/service/role_svc.rs
```

- [ ] **Step 2: 改 use + 每个函数签名 + body**

`use sqlx::MySqlPool;` → `use crate::db::DbPool;`

每个函数: `&MySqlPool` → `&DbPool`，外层包 match（MySQL 分支保留 body，其他分支 `Err(AppError::UnsupportedDb(pool.db_type()))`）。

`role_svc.rs:109` 的 `pool.begin()` 需在 MySQL 分支内改为 `p.begin()`。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/service/role_svc.rs
git commit -m "refactor(user-service): role_svc accepts DbPool"
```

---

## Task 10: 改 service 层 claim_svc.rs 签名

**Files:**
- Modify: `services/user-service/src/service/claim_svc.rs`

- [ ] **Step 1: 读 claim_svc.rs**

```bash
Read: services/user-service/src/service/claim_svc.rs
```

- [ ] **Step 2: 改 use + 每个函数签名 + body**

`use sqlx::MySqlPool;` → `use crate::db::DbPool;`

每个函数: `&MySqlPool` → `&DbPool`，外层包 match（MySQL 分支保留 body，其他分支 `Err(AppError::UnsupportedDb(pool.db_type()))`）。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/service/claim_svc.rs
git commit -m "refactor(user-service): claim_svc accepts DbPool"
```

---

## Task 11: 改 routes/health.rs match 四种池

**Files:**
- Modify: `services/user-service/src/routes/health.rs`

- [ ] **Step 1: 读 health.rs**

```bash
Read: services/user-service/src/routes/health.rs
```

- [ ] **Step 2: 把 `&MySqlPool` 改成 `&DbPool`，health 逻辑改 match**

示例:
```rust
pub async fn health(State(pool): State<DbPool>) -> impl IntoResponse {
    let db_ok = match &pool {
        DbPool::Mysql(p) => sqlx::query("SELECT 1").execute(p).await.is_ok(),
        DbPool::Mssql(p) => sqlx::query("SELECT 1").execute(p).await.is_ok(),
        DbPool::Sqlite(p) => sqlx::query("SELECT 1").execute(p).await.is_ok(),
        DbPool::Dameng(c) => c
            .lock()
            .ok()
            .and_then(|g| g.execute("SELECT 1", ()).ok())
            .is_some(),
    };
    // ... 返回逻辑
}
```

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/routes/health.rs
git commit -m "feat(user-service): health check matches all four DbPool variants"
```

---

## Task 12: 改 routes/users.rs、internal.rs、permissions.rs、roles.rs、user_roles.rs 签名

**Files:**
- Modify: `services/user-service/src/routes/users.rs`
- Modify: `services/user-service/src/routes/internal.rs`
- Modify: `services/user-service/src/routes/permissions.rs`
- Modify: `services/user-service/src/routes/roles.rs`
- Modify: `services/user-service/src/routes/user_roles.rs`

- [ ] **Step 1: 读五个文件确认 sqlx::MySqlPool 出现位置**

```bash
Grep: pattern="MySqlPool", path="services/user-service/src/routes", output_mode="content", -n=true
```

- [ ] **Step 2: 替换为 DbPool**

每个文件: `use sqlx::MySqlPool;` → `use user_service::db::DbPool;`

`State(pool): State<MySqlPool>` → `State(pool): State<DbPool>`

服务调用 `service_fn(&pool, ...)` 不变（已接受 DbPool）。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/routes/
git commit -m "refactor(user-service): routes layer state typed as DbPool"
```

---

## Task 13: 更新 config.yaml 加 db_type

**Files:**
- Modify: `services/user-service/config/config.yaml`

- [ ] **Step 1: 插入 db_type 字段**

`config/config.yaml` 的 `database:` 段下:
```yaml
database:
  db_type: mysql
  url: "mysql://user:pass@localhost:3306/kudi"
  max_connections: 16
```

- [ ] **Step 2: 提交**

```bash
git add services/user-service/config/config.yaml
git commit -m "feat(user-service): set db_type=mysql in config"
```

---

## Task 14: 整体编译 + 跑测试

**Files:** (无)

- [ ] **Step 1: 格式化**

```bash
cargo fmt -p user-service
```

- [ ] **Step 2: clippy 严格模式**

```bash
cargo clippy -p user-service --all-targets -- -D warnings
```

Expected: 0 warnings。

- [ ] **Step 3: 跑全部单测**

```bash
cargo test -p user-service --lib
```

Expected: 全部 PASS（含新加的 config::tests / error::tests）。

- [ ] **Step 4: 跑集成测试（仅 MySQL 分支）**

```bash
cargo test -p user-service --test integration_test
```

Expected: 现有用例全 PASS（无回归）。

---

## Task 15: 集成测试加 multi_db 模块

**Files:**
- Modify: `services/user-service/tests/integration_test.rs`

- [ ] **Step 1: 读文件末尾**

```bash
Read: services/user-service/tests/integration_test.rs
```

- [ ] **Step 2: 文件末尾追加 mod multi_db**

```rust
mod multi_db {
    use user_service::config::{DatabaseCfg, DbType};

    #[test]
    fn dameng_yaml_parses() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: dameng
  url: "Driver={DM8 ODBC DRIVER};Server=127.0.0.1;Port=5236;UID=SYSDBA;PWD=x;Schema=DMHR"
  max_connections: 1
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: user_service::config::Config =
            serde_yaml::from_str(yaml).expect("dameng cfg parse");
        assert_eq!(cfg.database.db_type, DbType::Dameng);
    }

    #[test]
    fn unsupported_db_response_is_501() {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        use user_service::config::DbType;
        use user_service::error::AppError;

        let resp = AppError::UnsupportedDb(DbType::Dameng).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
```

注：本模块不连真实数据库，仅验证配置解析和错误响应。

- [ ] **Step 3: 跑测试**

```bash
cargo test -p user-service --test integration_test multi_db
```

Expected: 2 PASS。

- [ ] **Step 4: 提交**

```bash
git add services/user-service/tests/integration_test.rs
git commit -m "test(user-service): add multi_db integration module"
```

---

## Task 16: 最终验证

**Files:** (无)

- [ ] **Step 1: 全量 cargo fmt + clippy**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 2: 全量 cargo test**

```bash
cargo test -p user-service
```

Expected: 全部 PASS；新加 config::tests（6 个）+ error::tests（1 个）+ integration_test::multi_db（2 个）均通过；无回归。

- [ ] **Step 3: 检查 git status**

```bash
git status
git log --oneline -15
```

Expected: 15 个新 commit（Task 1-15 各 1 个，Task 14 / 16 无 commit），前缀全部 `feat(user-service):` / `refactor(user-service):` / `test(user-service):` / `fix(user-service):`。
