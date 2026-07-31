# token-service 切换到 MySQL 存储 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `services/token-service` 从 Redis + `clients.json` 切到 MySQL，对外 HTTP API / JWT / JWKS 行为不变

**Architecture:** 沿用 user-service 的 `DbPool::Mysql(MySqlPool)` 枚举模式；4 张表（oauth_clients / auth_codes / revoked_jtis / refresh_markers）；路由薄、service 业务、store mysql 函数式 sqlx；MySQL session 强制 UTC；INSERT IGNORE 处理幂等

**Tech Stack:** Rust 2021, axum 0.7, sqlx 0.8 (mysql + aws_lc_rs tls, **无** macros), serde_yaml, thiserror, testcontainers 0.20

**Spec:** `docs/superpowers/specs/2026-06-11-token-service-mysql-storage-design.md`

**与 spec 的偏差**：
- spec §6 定义了 `ServiceError` 枚举（NotFound / Forbidden / UpstreamUnreachable / Db / Internal）。**本 plan 保持 route 层用 `Result<Json<T>, (StatusCode, &'static str)>`**，因为 HTTP 行为与 spec 表格完全一致（已逐行核对）；引入 ServiceError 枚举收益小（route 只一处转换），属过度抽象。如未来要统一错误处理，可单独立项。
- spec §5 提出 `src/store/mysql.rs` 文件；本 plan 中 service 层直接用 sqlx 函数式 API，`store/mysql.rs` 仅留占位说明（避免空文件造成的 cargo 警告）。

**前置**:
- 当前分支 `feat/e2e-login`（非 main）
- 本机已 `cargo` + PowerShell w/ `vcvarsall x64` 可用
- Docker daemon 在线（testcontainers 跑 MySQL 镜像）

**通用约定**（每步都遵守）:
- TDD：先写失败测试 → 跑确认失败 → 写实现 → 跑确认通过 → commit
- 不在 plan 中留 "TODO" / "类似 Task N" / "适当处理"
- 每次 commit 前 `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
- 集成测试 `cargo test -- --test-threads=1`
- 真实 PowerShell 跑（项目规则：沙箱不跑 cargo）

---

## Task 1: 更新 Cargo.toml 依赖

**Files:**
- Modify: `services/token-service/Cargo.toml`

- [ ] **Step 1: 删除 redis / config / dotenvy 依赖**

打开 `services/token-service/Cargo.toml`，在 `[dependencies]` 段**删除**这三行：
```toml
redis.workspace = true
config.workspace = true
dotenvy.workspace = true
```

- [ ] **Step 2: 加 sqlx / serde_yaml / anyhow**

在同一段**添加**：
```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls-aws-lc-rs", "mysql"] }
serde_yaml = "0.9"
anyhow = "1"
```

注意：sqlx **不**含 `macros` feature（项目规则 + 与 user-service 函数式风格一致）。

- [ ] **Step 3: 验证编译**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service 2>&1 | Tee-Object -FilePath tmp-cargo-check.log
```
预期：`error[E0432]: unresolved import 'redis' / 'config' / 'dotenvy'`（因为 `lib.rs` / `main.rs` 还在 import 旧依赖，先容忍这些错；本步只确认依赖本身能被解析）。

- [ ] **Step 4: commit**

```powershell
git add services/token-service/Cargo.toml
git commit -m "chore(token-service): swap redis/toml deps for sqlx/yaml in Cargo.toml"
```

---

## Task 2: 写 migrations/20260611000001_init.sql

**Files:**
- Create: `services/token-service/migrations/20260611000001_init.sql`

- [ ] **Step 1: 创建 migrations 目录**

```powershell
New-Item -ItemType Directory -Path services\token-service\migrations -Force
```

- [ ] **Step 2: 写 4 张表 DDL**

文件 `services/token-service/migrations/20260611000001_init.sql`：

```sql
-- 1) OAuth 客户端注册表
CREATE TABLE oauth_clients (
  client_id             VARCHAR(64)  PRIMARY KEY,
  client_secret_hash    VARCHAR(255) NOT NULL,
  name                  VARCHAR(128) NOT NULL,
  permissions           JSON         NOT NULL,
  grant_types           JSON         NOT NULL,
  access_token_ttl_sec  BIGINT       NOT NULL,
  refresh_token_ttl_sec BIGINT       NOT NULL,
  status                VARCHAR(16)  NOT NULL DEFAULT 'active',
  created_at            TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at            TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

-- 2) 授权码（替换 SET_EX prefix:code → user_id）
CREATE TABLE auth_codes (
  code        VARCHAR(128) PRIMARY KEY,
  user_id     VARCHAR(64)  NOT NULL,
  expires_at  TIMESTAMP    NOT NULL,
  consumed_at TIMESTAMP    NULL,
  created_at  TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_auth_codes_expires (expires_at)
);

-- 3) 吊销名单（替换 SET_EX prefix:blacklist:jti）
CREATE TABLE revoked_jtis (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMP    NOT NULL,
  created_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_revoked_expires (expires_at)
);

-- 4) refresh 标记（替换 SET_EX prefix:refresh:jti）
CREATE TABLE refresh_markers (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMP    NOT NULL,
  created_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_refresh_expires (expires_at)
);
```

- [ ] **Step 3: 不 commit（migration 随 Task 4 一起入库）**

> 说明：本任务单独 commit SQL 没意义，等 Task 4 的 `db.rs` 一起 commit。

---

## Task 3: tests/digua_common/mod.rs — testcontainers MySQL helper

**Files:**
- Create: `services/token-service/tests/digua_common/mod.rs`

- [ ] **Step 1: 写测试公共模块**

```rust
//! 集成测试公共 helper：起 MySQL 容器 + 跑 migrations
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;

pub const MIGRATION_SQL: &str = include_str!("../../migrations/20260611000001_init.sql");

pub async fn spawn_mysql() -> MySqlPool {
    let container = Mysql::default().start().await.expect("start mysql");
    let host = container.get_host().await.expect("mysql host");
    let port = container.get_host_port_ipv4(3306).await.expect("mysql port");
    let url = format!("mysql://root:root@{}:{}/test", host, port);

    let pool = MySqlPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await
        .expect("connect mysql");

    // 拆 ; 切 statement
    for stmt in MIGRATION_SQL.split(';') {
        let s = stmt.trim();
        if s.is_empty() || s.starts_with("--") {
            continue;
        }
        sqlx::query(s).execute(&pool).await.expect("run migration");
    }
    pool
}
```

- [ ] **Step 2: 暂时不 commit（helpers 跟第一个用到它的 test 一起 commit）**

---

## Task 4: src/db.rs — DbPool + init_pool

**Files:**
- Create: `services/token-service/src/db.rs`
- Modify: `services/token-service/src/lib.rs`（加 `pub mod db;`）

- [ ] **Step 1: 写 src/db.rs**

```rust
use anyhow::{Context, Result};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

use crate::config::DatabaseCfg;

#[derive(Clone)]
pub enum DbPool {
    Mysql(MySqlPool),
}

impl DbPool {
    pub async fn ping(&self) -> Result<()> {
        match self {
            Self::Mysql(p) => sqlx::query("SELECT 1").execute(p).await.context("ping")?,
        }
        Ok(())
    }
}

pub async fn init_pool(cfg: &DatabaseCfg) -> Result<DbPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(cfg.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .after_connect(|conn, _meta| Box::pin(async move {
            // 强制 UTC session，确保 NOW() 与 expires_at 一致
            sqlx::query("SET time_zone = '+00:00'")
                .execute(conn)
                .await?;
            Ok(())
        }))
        .connect(&cfg.url)
        .await
        .with_context(|| format!("connect mysql {}", cfg.url))?;
    Ok(DbPool::Mysql(pool))
}
```

- [ ] **Step 2: 在 src/lib.rs 加 pub mod db**

打开 `services/token-service/src/lib.rs`，在已有的 `pub mod config;` 之后加一行：
```rust
pub mod db;
```

- [ ] **Step 3: 写 src/db.rs 单元测试（验证 after_connect 设置 TZ）**

在 `src/db.rs` 末尾追加：
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DatabaseCfg;
    use crate::config::DbType;

    fn test_cfg() -> DatabaseCfg {
        DatabaseCfg {
            db_type: DbType::Mysql,
            url: std::env::var("TEST_MYSQL_URL")
                .unwrap_or_else(|_| "mysql://root:root@127.0.0.1:3306/test".into()),
            max_connections: 2,
        }
    }

    #[tokio::test]
    #[ignore = "需要真实 MySQL；testcontainers 走 tests/integration_test"]
    async fn init_sets_utc_timezone() {
        let pool = init_pool(&test_cfg()).await.unwrap();
        let row: (String,) = sqlx::query_as("SELECT @@session.time_zone")
            .fetch_one(match &pool { DbPool::Mysql(p) => p })
            .await
            .unwrap();
        assert_eq!(row.0, "+00:00");
    }
}
```

- [ ] **Step 4: 验证编译**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service
```
预期：`error[E0432]: unresolved import 'crate::config::DatabaseCfg'`（config.rs 还没建，先容忍）。

- [ ] **Step 5: commit**

```powershell
git add services/token-service/migrations services/token-service/src/db.rs services/token-service/src/lib.rs
git commit -m "feat(token-service): add db.rs DbPool + init_pool with UTC after_connect"
```

---

## Task 5: src/config.rs — YAML 配置 + DbType/DatabaseCfg

**Files:**
- Create: `services/token-service/src/config.rs`（覆盖原文件）
- Create: `services/token-service/config/config.yaml`

- [ ] **Step 1: 删除旧 src/config.rs 与 config/token-service.toml**

```powershell
Remove-Item services\token-service\src\config.rs
Remove-Item services\token-service\config\token-service.toml -ErrorAction SilentlyContinue
```

- [ ] **Step 2: 写新 src/config.rs**

```rust
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::str::FromStr;
use std::time::Duration as StdDuration;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Mysql,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCfg {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseCfg {
    pub db_type: DbType,
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ConsulCfg {
    pub addr: String,
    pub service_name: String,
    pub service_id_file: String,
}

impl Default for ConsulCfg {
    fn default() -> Self {
        Self {
            addr: "http://localhost:8500".into(),
            service_name: "token-service".into(),
            service_id_file: "/tmp/token-service-id".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerCfg,
    pub database: DatabaseCfg,
    pub consul: ConsulCfg,
    pub jwt_private_key_path: String,
    pub jwt_public_key_path: String,
    pub issuer: String,
    pub default_access_ttl_sec: i64,
    pub default_refresh_ttl_sec: i64,
    pub default_resource_ttl_sec: i64,
    pub user_service_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerCfg { host: "0.0.0.0".into(), port: 4000 },
            database: DatabaseCfg {
                db_type: DbType::Mysql,
                url: "mysql://token_service:token_pwd@localhost:3306/token_service".into(),
                max_connections: 16,
            },
            consul: ConsulCfg::default(),
            jwt_private_key_path: "keys/private.pem".into(),
            jwt_public_key_path: "keys/public.pem".into(),
            issuer: "superpowers-token-service".into(),
            default_access_ttl_sec: 3600,
            default_refresh_ttl_sec: 2592000,
            default_resource_ttl_sec: 300,
            user_service_url: "http://user-service:8080".into(),
        }
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let raw = fs::read_to_string(path).with_context(|| format!("read config {path}"))?;
    let cfg: Config = serde_yaml::from_str(&raw).with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mysql_config() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 4000 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.database.db_type, DbType::Mysql);
        assert_eq!(cfg.server.port, 4000);
        assert_eq!(cfg.database.max_connections, 8);
    }

    #[test]
    fn parse_invalid_db_type() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 4000 }
database:
  db_type: oracle
  url: "x"
  max_connections: 1
"#;
        assert!(serde_yaml::from_str::<Config>(yaml).is_err());
    }

    #[test]
    fn consul_default_applied() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 4000 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.consul.service_name, "token-service");
    }
}
```

- [ ] **Step 3: 写 config/config.yaml**

```yaml
server:
  host: "0.0.0.0"
  port: 4000
database:
  db_type: mysql
  url: "mysql://token_service:token_pwd@localhost:3306/token_service"
  max_connections: 16
consul:
  addr: "http://localhost:8500"
  service_name: "token-service"
  service_id_file: "/tmp/token-service-id"
jwt_private_key_path: "keys/private.pem"
jwt_public_key_path: "keys/public.pem"
issuer: "superpowers-token-service"
default_access_ttl_sec: 3600
default_refresh_ttl_sec: 2592000
default_resource_ttl_sec: 300
user_service_url: "http://user-service:8080"
```

- [ ] **Step 4: 在 src/lib.rs 把 `pub use config::TokenServerConfig` 改成 `pub use config::{Config as TokenServerConfig, DatabaseCfg, DbType, load_config}`**

打开 `src/lib.rs`，把现有 `pub use config::TokenServerConfig;` 替换为：
```rust
pub use config::{load_config, Config as TokenServerConfig, DatabaseCfg, DbType};
```

- [ ] **Step 5: 跑 config 单元测试**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --lib config::tests
```
预期：3 个 test 全 PASS。

- [ ] **Step 6: 验证编译**

```powershell
cargo check -p token-service
```
预期：可能仍报 `lib.rs:redis_conn` 等未定义（旧代码），先容忍。

- [ ] **Step 7: commit**

```powershell
git add services/token-service/src/config.rs services/token-service/src/lib.rs services/token-service/config/config.yaml
git commit -m "feat(token-service): switch config to YAML + DatabaseCfg with DbType enum"
```

---

## Task 6: src/secret.rs — 迁出 hash/upgrade 工具

**Files:**
- Create: `services/token-service/src/secret.rs`
- Modify: `services/token-service/src/lib.rs`（加 `pub mod secret;`）

- [ ] **Step 1: 写 src/secret.rs（保留原 store/client.rs 已有测试）**

```rust
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;

/// Dev 用的明文 client_secret；启动时如果现存 hash 验证不通过就用它重算并替换。
/// 生产环境应通过环境变量注入真 hash，本函数不参与。
pub const DEFAULT_DEV_CLIENT_SECRET: &str = "dev-gateway-secret";

pub fn hash_client_secret(plain: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .expect("hash client secret")
        .to_string()
}

/// 验证现存 hash 是否与默认明文一致；不一致则重算并原地替换。
pub fn maybe_upgrade_client_secret(existing_hash: &mut String, default_plain: &str) {
    let verified = PasswordHash::new(existing_hash)
        .ok()
        .and_then(|ph| {
            Argon2::default()
                .verify_password(default_plain.as_bytes(), &ph)
                .ok()
        })
        .is_some();
    if !verified {
        *existing_hash = hash_client_secret(default_plain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_dev_secret() {
        let h = hash_client_secret(DEFAULT_DEV_CLIENT_SECRET);
        assert!(PasswordHash::new(&h).is_ok());
    }

    #[test]
    fn upgrade_replaces_placeholder_hash() {
        let mut h = "$argon2id$v=19$m=65536,t=3,p=4$placeholder$invalidsaltvalue".into();
        maybe_upgrade_client_secret(&mut h, DEFAULT_DEV_CLIENT_SECRET);
        let ph = PasswordHash::new(&h).unwrap();
        assert!(Argon2::default()
            .verify_password(DEFAULT_DEV_CLIENT_SECRET.as_bytes(), &ph)
            .is_ok());
    }
}
```

- [ ] **Step 2: 在 src/lib.rs 加 pub mod secret**

在 `pub mod db;` 之后加：
```rust
pub mod secret;
```

- [ ] **Step 3: 跑单元测试**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --lib secret::tests
```
预期：2 个 test 全 PASS。

- [ ] **Step 4: commit**

```powershell
git add services/token-service/src/secret.rs services/token-service/src/lib.rs
git commit -m "refactor(token-service): extract hash_client_secret into src/secret.rs"
```

---

## Task 7: src/models — 4 个 FromRow struct

**Files:**
- Create: `services/token-service/src/models/mod.rs`
- Create: `services/token-service/src/models/oauth_client.rs`
- Create: `services/token-service/src/models/auth_code.rs`
- Create: `services/token-service/src/models/revoked_jti.rs`
- Create: `services/token-service/src/models/refresh_marker.rs`
- Modify: `services/token-service/src/lib.rs`（加 `pub mod models;`）

- [ ] **Step 1: 写 src/models/mod.rs**

```rust
pub mod auth_code;
pub mod oauth_client;
pub mod refresh_marker;
pub mod revoked_jti;
```

- [ ] **Step 2: 写 src/models/oauth_client.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClientRow {
    pub client_id: String,
    pub client_secret_hash: String,
    pub name: String,
    pub permissions: serde_json::Value, // JSON 列 -> Value
    pub grant_types: serde_json::Value,
    pub access_token_ttl_sec: i64,
    pub refresh_token_ttl_sec: i64,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub client_id: String,
    pub client_secret_hash: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub grant_types: Vec<String>,
    pub access_token_ttl_sec: i64,
    pub refresh_token_ttl_sec: i64,
    pub status: String,
}

impl ClientRow {
    pub fn into_dto(self) -> Client {
        Client {
            client_id: self.client_id,
            client_secret_hash: self.client_secret_hash,
            name: self.name,
            permissions: serde_json::from_value(self.permissions).unwrap_or_default(),
            grant_types: serde_json::from_value(self.grant_types).unwrap_or_default(),
            access_token_ttl_sec: self.access_token_ttl_sec,
            refresh_token_ttl_sec: self.refresh_token_ttl_sec,
            status: self.status,
        }
    }
}
```

- [ ] **Step 3: 写 src/models/auth_code.rs**

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthCodeRow {
    pub code: String,
    pub user_id: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub consumed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

- [ ] **Step 4: 写 src/models/revoked_jti.rs**

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RevokedJtiRow {
    pub jti: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

- [ ] **Step 5: 写 src/models/refresh_marker.rs**

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RefreshMarkerRow {
    pub jti: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

- [ ] **Step 6: 在 src/lib.rs 加 pub mod models**

在 `pub mod secret;` 之后加：
```rust
pub mod models;
```

- [ ] **Step 7: 加 chrono 依赖（如果还没有）**

打开 `services/token-service/Cargo.toml`，在 `[dependencies]` 加：
```toml
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 8: 验证编译**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service
```
预期：models 模块编译通过；可能仍有旧 lib.rs/main.rs/routes 引用 `redis_conn` 等未定义。

- [ ] **Step 9: commit**

```powershell
git add services/token-service/src/models services/token-service/src/lib.rs services/token-service/Cargo.toml
git commit -m "feat(token-service): add models/ with FromRow structs for 4 tables"
```

---

## Task 8: src/service/mod.rs + service/oauth_client_svc.rs (TDD)

**Files:**
- Create: `services/token-service/src/service/mod.rs`
- Create: `services/token-service/tests/oauth_client_test.rs`（先写，task 中跑）
- Create: `services/token-service/src/service/oauth_client_svc.rs`
- Modify: `services/token-service/src/lib.rs`（加 `pub mod service;`）

- [ ] **Step 1: 写 tests/oauth_client_test.rs（先失败）**

```rust
// requires: cargo test --test-threads=1
mod digua_common;

use token_service::db::DbPool;
use token_service::models::oauth_client::Client;
use token_service::service::oauth_client_svc;

fn sample_client(id: &str) -> Client {
    Client {
        client_id: id.to_string(),
        client_secret_hash: token_service::secret::hash_client_secret("dev-gateway-secret"),
        name: format!("{id}-name"),
        permissions: vec!["token:issue".into()],
        grant_types: vec!["authorization_code".into()],
        access_token_ttl_sec: 3600,
        refresh_token_ttl_sec: 2592000,
        status: "active".into(),
    }
}

#[tokio::test]
async fn seed_then_find_by_id() {
    let pool = digua_common::spawn_mysql().await;
    let n = oauth_client_svc::seed_default_clients(&pool).await.unwrap();
    assert_eq!(n, 4); // api-gateway / token-service / user-service / notification-service

    let c = oauth_client_svc::find_by_id(&pool, "api-gateway").await.unwrap();
    assert!(c.is_some());
    let c = c.unwrap();
    assert_eq!(c.client_id, "api-gateway");
    assert!(c.permissions.contains(&"token:issue".to_string()));
}

#[tokio::test]
async fn seed_is_idempotent() {
    let pool = digua_common::spawn_mysql().await;
    let n1 = oauth_client_svc::seed_default_clients(&pool).await.unwrap();
    let n2 = oauth_client_svc::seed_default_clients(&pool).await.unwrap();
    assert_eq!(n1, 4);
    assert_eq!(n2, 0); // 第二次应当 0 affected rows
}

#[tokio::test]
async fn find_unknown_returns_none() {
    let pool = digua_common::spawn_mysql().await;
    oauth_client_svc::seed_default_clients(&pool).await.unwrap();
    let c = oauth_client_svc::find_by_id(&pool, "ghost").await.unwrap();
    assert!(c.is_none());
}

#[tokio::test]
async fn insert_new_client() {
    let pool = digua_common::spawn_mysql().await;
    let c = sample_client("svc-new");
    oauth_client_svc::insert(&pool, &c).await.unwrap();
    let got = oauth_client_svc::find_by_id(&pool, "svc-new").await.unwrap();
    assert!(got.is_some());
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test oauth_client_test -- --test-threads=1
```
预期：编译错误（service/oauth_client_svc.rs 还没建），属正常 RED。

- [ ] **Step 3: 写 src/service/mod.rs**

```rust
pub mod auth_code_svc;
pub mod oauth_client_svc;
pub mod refresh_svc;
pub mod revoke_svc;
```

> 其他三个 svc 暂为空 stub，下个 task 填充。

- [ ] **Step 4: 写 src/service/oauth_client_svc.rs**

```rust
use crate::db::DbPool;
use crate::models::oauth_client::{Client, ClientRow};
use crate::secret::{hash_client_secret, DEFAULT_DEV_CLIENT_SECRET};

pub async fn find_by_id(pool: &DbPool, client_id: &str) -> Result<Option<Client>, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let row: Option<ClientRow> = sqlx::query_as(
                "SELECT client_id, client_secret_hash, name, permissions, grant_types, \
                        access_token_ttl_sec, refresh_token_ttl_sec, status, created_at, updated_at \
                 FROM oauth_clients WHERE client_id = ?",
            )
            .bind(client_id)
            .fetch_optional(p)
            .await?;
            Ok(row.map(|r| r.into_dto()))
        }
    }
}

pub async fn insert(pool: &DbPool, c: &Client) -> Result<(), sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let perms = serde_json::to_value(&c.permissions).unwrap_or(serde_json::json!([]));
            let grants = serde_json::to_value(&c.grant_types).unwrap_or(serde_json::json!([]));
            sqlx::query(
                "INSERT INTO oauth_clients \
                 (client_id, client_secret_hash, name, permissions, grant_types, \
                  access_token_ttl_sec, refresh_token_ttl_sec, status) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&c.client_id)
            .bind(&c.client_secret_hash)
            .bind(&c.name)
            .bind(perms)
            .bind(grants)
            .bind(c.access_token_ttl_sec)
            .bind(c.refresh_token_ttl_sec)
            .bind(&c.status)
            .execute(p)
            .await?;
            Ok(())
        }
    }
}

fn default_clients() -> Vec<Client> {
    let hash = hash_client_secret(DEFAULT_DEV_CLIENT_SECRET);
    vec![
        Client {
            client_id: "api-gateway".into(),
            client_secret_hash: hash.clone(),
            name: "API Gateway".into(),
            permissions: vec!["token:issue".into(), "token:revoke".into()],
            grant_types: vec!["authorization_code".into(), "refresh_token".into()],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
        },
        Client {
            client_id: "token-service".into(),
            client_secret_hash: hash.clone(),
            name: "Token Service".into(),
            permissions: vec![
                "token:issue".into(),
                "token:revoke".into(),
                "token:refresh".into(),
                "user:read".into(),
            ],
            grant_types: vec!["authorization_code".into(), "refresh_token".into()],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
        },
        Client {
            client_id: "user-service".into(),
            client_secret_hash: hash.clone(),
            name: "User Service".into(),
            permissions: vec![
                "user:read".into(),
                "user:write".into(),
                "role:read".into(),
                "permission:read".into(),
            ],
            grant_types: vec!["client_credentials".into()],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
        },
        Client {
            client_id: "notification-service".into(),
            client_secret_hash: hash,
            name: "Notification Service".into(),
            permissions: vec!["notify:send".into(), "notify:read".into()],
            grant_types: vec!["client_credentials".into()],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
        },
    ]
}

/// 空表时插入 4 条默认客户端；返回 affected rows。已存在则 0。
pub async fn seed_default_clients(pool: &DbPool) -> Result<u64, sqlx::Error> {
    let n: (i64,) = match pool {
        DbPool::Mysql(p) => sqlx::query_as("SELECT COUNT(*) FROM oauth_clients")
            .fetch_one(p)
            .await?,
    };
    if n.0 > 0 {
        return Ok(0);
    }
    for c in default_clients() {
        insert(pool, &c).await?;
    }
    Ok(4)
}
```

- [ ] **Step 5: 在 src/lib.rs 加 pub mod service**

在 `pub mod models;` 之后加：
```rust
pub mod service;
```

- [ ] **Step 6: 跑测试确认通过**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test oauth_client_test -- --test-threads=1
```
预期：4 个 test 全 PASS。

- [ ] **Step 7: commit**

```powershell
git add services/token-service/tests/digua_common services/token-service/tests/oauth_client_test.rs services/token-service/src/service services/token-service/src/lib.rs
git commit -m "feat(token-service): oauth_client_svc with seed_default_clients + find/insert"
```

---

## Task 9: src/service/auth_code_svc.rs (TDD, 原子两步 consume)

**Files:**
- Create: `services/token-service/tests/auth_code_test.rs`
- Create: `services/token-service/src/service/auth_code_svc.rs`

- [ ] **Step 1: 写 tests/auth_code_test.rs**

```rust
// requires: cargo test --test-threads=1
mod digua_common;

use token_service::db::DbPool;
use token_service::service::auth_code_svc;

#[tokio::test]
async fn save_then_consume_succeeds_once() {
    let pool = digua_common::spawn_mysql().await;
    auth_code_svc::save(&pool, "code-1", "user-1", 3600).await.unwrap();
    let u = auth_code_svc::consume(&pool, "code-1").await.unwrap();
    assert_eq!(u, Some("user-1".to_string()));
}

#[tokio::test]
async fn second_consume_returns_none() {
    let pool = digua_common::spawn_mysql().await;
    auth_code_svc::save(&pool, "code-2", "user-2", 3600).await.unwrap();
    let _ = auth_code_svc::consume(&pool, "code-2").await.unwrap();
    let u2 = auth_code_svc::consume(&pool, "code-2").await.unwrap();
    assert_eq!(u2, None);
}

#[tokio::test]
async fn consume_expired_returns_none() {
    let pool = digua_common::spawn_mysql().await;
    auth_code_svc::save(&pool, "code-3", "user-3", 1).await.unwrap();
    // 让它过期
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let u = auth_code_svc::consume(&pool, "code-3").await.unwrap();
    assert_eq!(u, None);
}

#[tokio::test]
async fn consume_unknown_returns_none() {
    let pool = digua_common::spawn_mysql().await;
    let u = auth_code_svc::consume(&pool, "nope").await.unwrap();
    assert_eq!(u, None);
}

#[tokio::test]
async fn concurrent_consume_only_one_succeeds() {
    let pool = digua_common::spawn_mysql().await;
    auth_code_svc::save(&pool, "code-4", "user-4", 3600).await.unwrap();
    let p1 = pool.clone();
    let p2 = pool.clone();
    let (r1, r2) = tokio::join!(
        auth_code_svc::consume(&p1, "code-4"),
        auth_code_svc::consume(&p2, "code-4"),
    );
    let r1 = r1.unwrap();
    let r2 = r2.unwrap();
    let hits = [&r1, &r2].iter().filter(|x| x.is_some()).count();
    assert_eq!(hits, 1, "exactly one consumer must win");
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test auth_code_test -- --test-threads=1
```
预期：编译错误（auth_code_svc.rs 还没建）。

- [ ] **Step 3: 写 src/service/auth_code_svc.rs**

```rust
use crate::db::DbPool;

/// 写入新授权码，TTL 单位秒。重复 code 触发 1062 → 视为已存在（no-op）。
pub async fn save(
    pool: &DbPool,
    code: &str,
    user_id: &str,
    ttl_sec: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            sqlx::query(
                "INSERT INTO auth_codes (code, user_id, expires_at) \
                 VALUES (?, ?, DATE_ADD(NOW(), INTERVAL ? SECOND))",
            )
            .bind(code)
            .bind(user_id)
            .bind(ttl_sec)
            .execute(p)
            .await
            .or_else(|e| match e {
                sqlx::Error::Database(ref d) if d.code().as_deref() == Some("23000") => {
                    Ok(sqlx::mysql::MySqlQueryResult::default())
                }
                other => Err(other),
            })?;
            Ok(())
        }
    }
}

/// 原子两步 consume：
///   1. UPDATE ... SET consumed_at = NOW() WHERE 未消费 + 未过期
///   2. SELECT user_id
/// 返回 Some(user_id) 当且仅当第一步成功。
pub async fn consume(pool: &DbPool, code: &str) -> Result<Option<String>, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let res = sqlx::query(
                "UPDATE auth_codes SET consumed_at = NOW() \
                 WHERE code = ? AND consumed_at IS NULL AND expires_at > NOW()",
            )
            .bind(code)
            .execute(p)
            .await?;
            if res.rows_affected() == 0 {
                return Ok(None);
            }
            let row: Option<(String,)> =
                sqlx::query_as("SELECT user_id FROM auth_codes WHERE code = ?")
                    .bind(code)
                    .fetch_optional(p)
                    .await?;
            Ok(row.map(|(u,)| u))
        }
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test auth_code_test -- --test-threads=1
```
预期：5 个 test 全 PASS。

- [ ] **Step 5: commit**

```powershell
git add services/token-service/tests/auth_code_test.rs services/token-service/src/service/auth_code_svc.rs
git commit -m "feat(token-service): auth_code_svc with atomic two-step consume"
```

---

## Task 10: src/service/revoke_svc.rs (TDD, INSERT IGNORE 幂等)

**Files:**
- Create: `services/token-service/tests/revoke_test.rs`
- Create: `services/token-service/src/service/revoke_svc.rs`

- [ ] **Step 1: 写 tests/revoke_test.rs**

```rust
// requires: cargo test --test-threads=1
mod digua_common;

use token_service::db::DbPool;
use token_service::service::revoke_svc;

#[tokio::test]
async fn revoke_then_is_revoked() {
    let pool = digua_common::spawn_mysql().await;
    revoke_svc::revoke(&pool, "jti-1", 3600).await.unwrap();
    assert!(revoke_svc::is_revoked(&pool, "jti-1").await.unwrap());
}

#[tokio::test]
async fn revoke_is_idempotent() {
    let pool = digua_common::spawn_mysql().await;
    revoke_svc::revoke(&pool, "jti-2", 3600).await.unwrap();
    revoke_svc::revoke(&pool, "jti-2", 3600).await.unwrap(); // 不报错
    assert!(revoke_svc::is_revoked(&pool, "jti-2").await.unwrap());
}

#[tokio::test]
async fn unknown_jti_is_not_revoked() {
    let pool = digua_common::spawn_mysql().await;
    assert!(!revoke_svc::is_revoked(&pool, "nope").await.unwrap());
}

#[tokio::test]
async fn expired_revocation_returns_false() {
    let pool = digua_common::spawn_mysql().await;
    revoke_svc::revoke(&pool, "jti-3", 1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    assert!(!revoke_svc::is_revoked(&pool, "jti-3").await.unwrap());
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test revoke_test -- --test-threads=1
```
预期：编译错误。

- [ ] **Step 3: 写 src/service/revoke_svc.rs**

```rust
use crate::db::DbPool;

pub async fn revoke(pool: &DbPool, jti: &str, ttl_sec: i64) -> Result<(), sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            sqlx::query(
                "INSERT IGNORE INTO revoked_jtis (jti, expires_at) \
                 VALUES (?, DATE_ADD(NOW(), INTERVAL ? SECOND))",
            )
            .bind(jti)
            .bind(ttl_sec)
            .execute(p)
            .await?;
            Ok(())
        }
    }
}

pub async fn is_revoked(pool: &DbPool, jti: &str) -> Result<bool, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT 1 FROM revoked_jtis WHERE jti = ? AND expires_at > NOW() LIMIT 1",
            )
            .bind(jti)
            .fetch_optional(p)
            .await?;
            Ok(row.is_some())
        }
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test revoke_test -- --test-threads=1
```
预期：4 个 test 全 PASS。

- [ ] **Step 5: commit**

```powershell
git add services/token-service/tests/revoke_test.rs services/token-service/src/service/revoke_svc.rs
git commit -m "feat(token-service): revoke_svc with INSERT IGNORE idempotent revoke"
```

---

## Task 11: src/service/refresh_svc.rs (TDD, INSERT IGNORE 幂等)

**Files:**
- Create: `services/token-service/tests/refresh_marker_test.rs`
- Create: `services/token-service/src/service/refresh_svc.rs`

- [ ] **Step 1: 写 tests/refresh_marker_test.rs**

```rust
// requires: cargo test --test-threads=1
mod digua_common;

use token_service::db::DbPool;
use token_service::service::refresh_svc;

#[tokio::test]
async fn mark_then_is_marked() {
    let pool = digua_common::spawn_mysql().await;
    refresh_svc::mark(&pool, "jti-A", 3600).await.unwrap();
    assert!(refresh_svc::is_marked(&pool, "jti-A").await.unwrap());
}

#[tokio::test]
async fn mark_is_idempotent() {
    let pool = digua_common::spawn_mysql().await;
    refresh_svc::mark(&pool, "jti-B", 3600).await.unwrap();
    refresh_svc::mark(&pool, "jti-B", 3600).await.unwrap();
    assert!(refresh_svc::is_marked(&pool, "jti-B").await.unwrap());
}

#[tokio::test]
async fn unknown_jti_not_marked() {
    let pool = digua_common::spawn_mysql().await;
    assert!(!refresh_svc::is_marked(&pool, "nope").await.unwrap());
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test refresh_marker_test -- --test-threads=1
```
预期：编译错误。

- [ ] **Step 3: 写 src/service/refresh_svc.rs**

```rust
use crate::db::DbPool;

pub async fn mark(pool: &DbPool, jti: &str, ttl_sec: i64) -> Result<(), sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            sqlx::query(
                "INSERT IGNORE INTO refresh_markers (jti, expires_at) \
                 VALUES (?, DATE_ADD(NOW(), INTERVAL ? SECOND))",
            )
            .bind(jti)
            .bind(ttl_sec)
            .execute(p)
            .await?;
            Ok(())
        }
    }
}

pub async fn is_marked(pool: &DbPool, jti: &str) -> Result<bool, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT 1 FROM refresh_markers WHERE jti = ? AND expires_at > NOW() LIMIT 1",
            )
            .bind(jti)
            .fetch_optional(p)
            .await?;
            Ok(row.is_some())
        }
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test refresh_marker_test -- --test-threads=1
```
预期：3 个 test 全 PASS。

- [ ] **Step 5: commit**

```powershell
git add services/token-service/tests/refresh_marker_test.rs services/token-service/src/service/refresh_svc.rs
git commit -m "feat(token-service): refresh_svc with INSERT IGNORE idempotent mark"
```

---

## Task 12: src/lib.rs — TokenServerState 改造

**Files:**
- Modify: `services/token-service/src/lib.rs`

- [ ] **Step 1: 替换 TokenServerState**

打开 `src/lib.rs`，把整个 `TokenServerState` 结构体替换为：
```rust
use crate::db::DbPool;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;

pub type TokenHttpClient = Client<HttpConnector, Full<Bytes>>;

pub async fn fetch_json(
    client: &TokenHttpClient,
    url: &str,
) -> Result<serde_json::Value, String> {
    let req = Request::builder()
        .method("GET")
        .uri(url)
        .body(Full::new(Bytes::new()))
        .map_err(|e| format!("build request: {}", e))?;
    let resp = client.request(req).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| e.to_string())?
        .to_bytes();
    serde_json::from_slice(&body).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct TokenServerState {
    pub db: DbPool,
    pub issuer: std::sync::Arc<crate::issuer::TokenIssuer>,
    pub issuer_str: String,
    pub jwt_public_key: String,
    pub default_resource_ttl_sec: i64,
    pub http_client: TokenHttpClient,
    pub user_service_url: String,
}

pub mod config;
pub mod consul;
pub mod db;
pub mod issuer;
pub mod models;
pub mod secret;
pub mod service;
pub mod routes;

pub use config::{load_config, Config as TokenServerConfig, DatabaseCfg, DbType};
pub use consul::ConsulRegistrar;
pub use issuer::TokenIssuer;
```

- [ ] **Step 2: 验证编译（容忍 routes 报错）**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service 2>&1 | Tee-Object -FilePath tmp-cargo.log
```
预期：可能报 routes 用了 `state.client_store` / `state.redis_conn` 等未定义字段，下个 task 修。

- [ ] **Step 3: commit**

```powershell
git add services/token-service/src/lib.rs
git commit -m "feat(token-service): TokenServerState now holds DbPool instead of MultiplexedConnection"
```

---

## Task 13: src/main.rs — 加载 yaml / init_pool / seed

**Files:**
- Modify: `services/token-service/src/main.rs`

- [ ] **Step 1: 重写 main.rs**

完整文件内容：
```rust
use std::sync::Arc;
use token_service::routes;
use token_service::service::oauth_client_svc;
use token_service::{
    load_config, ConsulRegistrar, TokenIssuer, TokenServerConfig, TokenServerState,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "token_service=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg: TokenServerConfig = load_config("config/config.yaml")
        .or_else(|_| load_config("config/token-service.yaml"))
        .expect("failed to load config.yaml");

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

    let db = token_service::db::init_pool(&cfg.database)
        .await
        .expect("failed to connect to mysql");
    tracing::info!("connected to mysql at {}", cfg.database.url);

    let seeded = oauth_client_svc::seed_default_clients(&db)
        .await
        .expect("seed default clients");
    if seeded > 0 {
        tracing::info!("seeded {} default oauth clients", seeded);
    }

    let private_key_pem = std::fs::read_to_string(&cfg.jwt_private_key_path)
        .expect("failed to read private key");
    let public_key_pem = std::fs::read_to_string(&cfg.jwt_public_key_path)
        .expect("failed to read public key");

    let issuer = Arc::new(
        TokenIssuer::new(&private_key_pem, &cfg.issuer)
            .expect("failed to create token issuer"),
    );

    let state = TokenServerState {
        db: db.clone(),
        issuer,
        issuer_str: cfg.issuer.clone(),
        jwt_public_key: public_key_pem,
        default_resource_ttl_sec: cfg.default_resource_ttl_sec,
        http_client: {
            use hyper_util::client::legacy::connect::HttpConnector;
            use hyper_util::client::legacy::Client;
            use hyper_util::rt::TokioExecutor;
            Client::builder(TokioExecutor::new()).build(HttpConnector::new())
        },
        user_service_url: cfg.user_service_url.clone(),
    };

    let app = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .route(
            "/api/tokens/access",
            axum::routing::post(routes::access::handle_access),
        )
        .route(
            "/api/tokens/refresh",
            axum::routing::post(routes::refresh::handle_refresh),
        )
        .route(
            "/api/tokens/revoke",
            axum::routing::post(routes::revoke::handle_revoke),
        )
        .route(
            "/api/tokens/resource",
            axum::routing::post(routes::resource::handle_resource),
        )
        .route(
            "/api/tokens/.well-known/jwks.json",
            axum::routing::get(routes::jwks::handle_jwks),
        )
        .route(
            "/.well-known/jwks.json",
            axum::routing::get(routes::jwks::handle_jwks),
        )
        .route(
            "/api/api-keys",
            axum::routing::post(routes::api_key::handle_create_api_key),
        )
        .route(
            "/api/v1/tokens/user-jwt",
            axum::routing::post(routes::user_jwt::handle_user_jwt),
        )
        .with_state(state);

    let consul_registrar = ConsulRegistrar::new(&cfg.consul);
    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    match consul_registrar
        .register(&cfg.server.host, cfg.server.port)
        .await
    {
        Ok(_) => tracing::info!("token-service registered with consul"),
        Err(e) => tracing::warn!("failed to register with consul: {}", e),
    }

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("failed to listen for ctrl-c");
        tracing::info!("received shutdown signal, deregistering from consul");
        let _ = consul_registrar.deregister().await;
        std::process::exit(0);
    });

    tracing::info!("token-service listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 2: 验证编译（容忍 routes 报错）**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service 2>&1 | Tee-Object -FilePath tmp-cargo.log
```
预期：routes 还引用 `state.client_store` / `redis_conn` 等，下个 task 修。

- [ ] **Step 3: commit**

```powershell
git add services/token-service/src/main.rs
git commit -m "feat(token-service): main.rs loads yaml, init_pool, seeds 4 clients"
```

---

## Task 14: src/routes/access.rs — 改调 service

**Files:**
- Modify: `services/token-service/src/routes/access.rs`

- [ ] **Step 1: 重写 access.rs**

完整文件内容：
```rust
use crate::service::{auth_code_svc, oauth_client_svc};
use crate::TokenServerState;
use argon2::{PasswordHash, PasswordVerifier};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AccessRequest {
    pub grant_type: String,
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize)]
pub struct AccessResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

pub async fn handle_access(
    State(state): State<TokenServerState>,
    Json(req): Json<AccessRequest>,
) -> Result<Json<AccessResponse>, (StatusCode, &'static str)> {
    if req.grant_type != "authorization_code" {
        return Err((StatusCode::BAD_REQUEST, "unsupported grant_type"));
    }

    let client = oauth_client_svc::find_by_id(&state.db, &req.client_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "invalid client"))?;

    if client.status != "active" {
        return Err((StatusCode::FORBIDDEN, "client disabled"));
    }

    let password_verified = PasswordHash::new(&client.client_secret_hash)
        .ok()
        .and_then(|ph| {
            argon2::Argon2::default()
                .verify_password(req.client_secret.as_bytes(), &ph)
                .ok()
        })
        .is_some();
    if !password_verified {
        return Err((StatusCode::UNAUTHORIZED, "invalid client"));
    }

    let user_id = auth_code_svc::consume(&state.db, &req.code)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
        .ok_or((StatusCode::BAD_REQUEST, "invalid or expired code"))?;

    let claims_url = format!("{}/internal/users/{}/claims", state.user_service_url, user_id);
    let extra_claims: serde_json::Value = crate::fetch_json(&state.http_client, &claims_url)
        .await
        .map_err(|e| {
            tracing::warn!("user-service unreachable at {}: {}", claims_url, e);
            (StatusCode::SERVICE_UNAVAILABLE, "user_service_unreachable")
        })?;

    let issuer = &state.issuer;
    let at = issuer
        .issue_access_token(
            &user_id,
            &req.client_id,
            extra_claims,
            client.access_token_ttl_sec,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    let (rt, _family) = issuer
        .issue_refresh_token(&user_id, client.refresh_token_ttl_sec)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    Ok(Json(AccessResponse {
        access_token: at,
        refresh_token: rt,
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}
```

- [ ] **Step 2: 验证编译**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service 2>&1 | Tee-Object -FilePath tmp-cargo.log
```
预期：refresh.rs / revoke.rs / api_key.rs 仍报旧字段错。

- [ ] **Step 3: commit**

```powershell
git add services/token-service/src/routes/access.rs
git commit -m "refactor(token-service): access route uses oauth_client_svc + auth_code_svc"
```

---

## Task 15: src/routes/refresh.rs — 改调 service

**Files:**
- Modify: `services/token-service/src/routes/refresh.rs`

- [ ] **Step 1: 重写 refresh.rs**

完整文件内容：
```rust
use crate::service::{oauth_client_svc, refresh_svc};
use crate::TokenServerState;
use argon2::{PasswordHash, PasswordVerifier};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use jwt_simple::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

struct RefreshTokenInfo {
    sub: String,
    jti: String,
    exp: i64,
    iat: i64,
}

fn decode_refresh_token(
    token: &str,
    public_key_pem: &str,
    issuer: &str,
) -> Result<RefreshTokenInfo, StatusCode> {
    let public_key = RS256PublicKey::from_pem(public_key_pem)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let options = VerificationOptions {
        allowed_issuers: Some(std::collections::HashSet::from([issuer.to_string()])),
        accept_future: false,
        ..Default::default()
    };

    let claims = public_key
        .verify_token::<HashMap<String, serde_json::Value>>(token, Some(options))
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let sub = claims.subject.clone().unwrap_or_default();
    let jti = claims.jwt_id.clone().unwrap_or_default();
    let iat = claims.issued_at.map(|d| d.as_secs() as i64).unwrap_or(0);
    let exp = claims.expires_at.map(|d| d.as_secs() as i64).unwrap_or(0);

    Ok(RefreshTokenInfo { sub, jti, exp, iat })
}

pub async fn handle_refresh(
    State(state): State<TokenServerState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, (StatusCode, &'static str)> {
    let client = oauth_client_svc::find_by_id(&state.db, &req.client_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "invalid client"))?;

    let password_verified = PasswordHash::new(&client.client_secret_hash)
        .ok()
        .and_then(|ph| {
            argon2::Argon2::default()
                .verify_password(req.client_secret.as_bytes(), &ph)
                .ok()
        })
        .is_some();
    if !password_verified {
        return Err((StatusCode::UNAUTHORIZED, "invalid client"));
    }

    let info = decode_refresh_token(&req.refresh_token, &state.jwt_public_key, &state.issuer_str)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid or expired refresh_token"))?;

    let replayed = refresh_svc::is_marked(&state.db, &info.jti)
        .await
        .unwrap_or(false);
    if replayed {
        return Err((StatusCode::UNAUTHORIZED, "refresh token already used"));
    }

    let ttl = info.exp - info.iat;
    let _ = refresh_svc::mark(&state.db, &info.jti, ttl.max(0)).await;

    let claims_url = format!("{}/internal/users/{}/claims", state.user_service_url, info.sub);
    let extra_claims: serde_json::Value = match crate::fetch_json(&state.http_client, &claims_url).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("refresh: user-service unreachable: {}", e);
            return Err((StatusCode::SERVICE_UNAVAILABLE, "user_service_unreachable"));
        }
    };

    let issuer = &state.issuer;
    let at = issuer
        .issue_access_token(
            &info.sub,
            &req.client_id,
            extra_claims,
            client.access_token_ttl_sec,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    let (rt, _family) = issuer
        .issue_refresh_token(&info.sub, client.refresh_token_ttl_sec)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    Ok(Json(RefreshResponse {
        access_token: at,
        refresh_token: rt,
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}
```

- [ ] **Step 2: 验证编译**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service 2>&1 | Tee-Object -FilePath tmp-cargo.log
```

- [ ] **Step 3: commit**

```powershell
git add services/token-service/src/routes/refresh.rs
git commit -m "refactor(token-service): refresh route uses refresh_svc.is_marked/mark"
```

---

## Task 16: src/routes/revoke.rs — 改调 service

**Files:**
- Modify: `services/token-service/src/routes/revoke.rs`

- [ ] **Step 1: 重写 revoke.rs**

完整文件内容：
```rust
use crate::service::revoke_svc;
use crate::TokenServerState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token_jti: String,
    pub expires_in: i64,
}

#[derive(Debug, Serialize)]
pub struct RevokeResponse {
    pub success: bool,
}

pub async fn handle_revoke(
    State(state): State<TokenServerState>,
    Json(req): Json<RevokeRequest>,
) -> Result<Json<RevokeResponse>, (StatusCode, &'static str)> {
    revoke_svc::revoke(&state.db, &req.token_jti, req.expires_in.max(0))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    tracing::info!("revoked token jti={}", req.token_jti);
    Ok(Json(RevokeResponse { success: true }))
}
```

- [ ] **Step 2: 验证编译**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service 2>&1 | Tee-Object -FilePath tmp-cargo.log
```

- [ ] **Step 3: commit**

```powershell
git add services/token-service/src/routes/revoke.rs
git commit -m "refactor(token-service): revoke route uses revoke_svc.revoke (INSERT IGNORE)"
```

---

## Task 17: src/routes/api_key.rs — 改调 service

**Files:**
- Modify: `services/token-service/src/routes/api_key.rs`

- [ ] **Step 1: 重写 handler（保留原 DTO：client_id/client_secret 由后端 uuid 生成，TTL 字段 Option）**

完整文件内容：
```rust
use crate::models::oauth_client::Client;
use crate::service::oauth_client_svc;
use crate::TokenServerState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiKeyCreated {
    pub api_key_id: String,
    pub api_key_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub permissions: Vec<String>,
    pub grant_types: Vec<String>,
    pub access_token_ttl_sec: Option<i64>,
    pub refresh_token_ttl_sec: Option<i64>,
}

pub async fn handle_create_api_key(
    State(state): State<TokenServerState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<ApiKeyCreated>, (StatusCode, &'static str)> {
    let client_id = uuid::Uuid::new_v4().to_string();
    let client_secret = uuid::Uuid::new_v4().to_string();
    let hash = crate::secret::hash_client_secret(&client_secret);

    let new_client = Client {
        client_id: client_id.clone(),
        client_secret_hash: hash,
        name: req.name,
        permissions: req.permissions,
        grant_types: req.grant_types,
        access_token_ttl_sec: req.access_token_ttl_sec.unwrap_or(3600),
        refresh_token_ttl_sec: req.refresh_token_ttl_sec.unwrap_or(2592000),
        status: "active".into(),
    };

    oauth_client_svc::insert(&state.db, &new_client)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    tracing::info!("created api key client_id={}", client_id);

    Ok(Json(ApiKeyCreated {
        api_key_id: client_id,
        api_key_secret: client_secret,
    }))
}
```

- [ ] **Step 2: 验证编译**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service
```
预期：编译成功（`mod.rs` 已不再引用 `redis` / `client`）。

- [ ] **Step 3: commit**

```powershell
git add services/token-service/src/routes/api_key.rs
git commit -m "refactor(token-service): api_key route persists via oauth_client_svc.insert"
```

---

## Task 18: 删除旧 Redis / ClientStore / redis_test

**Files:**
- Delete: `services/token-service/src/store/redis.rs`
- Delete: `services/token-service/src/store/client.rs`
- Delete: `services/token-service/tests/redis_test.rs`
- Delete: `services/token-service/src/store/mod.rs`（只导出 mysql，redis/client 已删）

- [ ] **Step 1: 删 4 个文件**

```powershell
Remove-Item services\token-service\src\store\redis.rs
Remove-Item services\token-service\src\store\client.rs
Remove-Item services\token-service\tests\redis_test.rs
```

- [ ] **Step 2: 改 src/store/mod.rs 为只导出 mysql**

完整内容：
```rust
pub mod mysql;
```

> 注：本 plan 不单独写 `store/mysql.rs` 内容，因为 service 层已直接用 sqlx 函数式调用；如未来要抽 store 层再补。

- [ ] **Step 3: 创建 src/store/mysql.rs 占位**

```rust
//! 占位：service 层直接用 sqlx 函数式 API；如未来需抽 store 层，
//! 在此文件统一封装。当前内容仅导出空 pub。
```

- [ ] **Step 4: 验证编译**

```powershell
cd d:\projects\coder-micro-services
cargo check -p token-service
```
预期：编译成功。

- [ ] **Step 5: commit**

```powershell
git add -A services/token-service/src/store services/token-service/tests/redis_test.rs
git commit -m "refactor(token-service): remove redis + ClientStore; service layer calls sqlx directly"
```

---

## Task 19: tests/integration_test.rs — 端到端 MySQL 容器

**Files:**
- Modify: `services/token-service/tests/integration_test.rs`

- [ ] **Step 1: 写 integration_test.rs**

```rust
// requires: cargo test --test-threads=1
mod digua_common;

use token_service::db::{init_pool, DbPool};
use token_service::models::oauth_client::Client;
use token_service::service::{auth_code_svc, oauth_client_svc, refresh_svc, revoke_svc};

#[tokio::test]
async fn e2e_access_refresh_revoke() {
    let pool = digua_common::spawn_mysql().await;
    oauth_client_svc::seed_default_clients(&pool).await.unwrap();

    // 1. seed an auth code
    auth_code_svc::save(&pool, "auth-code-1", "user-42", 3600)
        .await
        .unwrap();

    // 2. consume it
    let u = auth_code_svc::consume(&pool, "auth-code-1").await.unwrap();
    assert_eq!(u, Some("user-42".into()));

    // 3. issue a refresh marker
    refresh_svc::mark(&pool, "refresh-jti-1", 3600).await.unwrap();
    assert!(refresh_svc::is_marked(&pool, "refresh-jti-1").await.unwrap());

    // 4. revoke a jti
    revoke_svc::revoke(&pool, "access-jti-1", 3600).await.unwrap();
    assert!(revoke_svc::is_revoked(&pool, "access-jti-1").await.unwrap());

    // 5. find api-gateway client
    let c = oauth_client_svc::find_by_id(&pool, "api-gateway").await.unwrap();
    assert!(c.is_some());
}
```

- [ ] **Step 2: 跑测试**

```powershell
cd d:\projects\coder-micro-services
cargo test -p token-service --test integration_test -- --test-threads=1
```
预期：1 个 e2e test PASS。

- [ ] **Step 3: commit**

```powershell
git add services/token-service/tests/integration_test.rs
git commit -m "test(token-service): integration_test runs full e2e against MySQL container"
```

---

## Task 20: docker-compose 启动顺序

**Files:**
- Modify: `infrastructure/docker-compose.*.yaml`（找到现有 token-service 的 compose 文件）

- [ ] **Step 1: 找到 compose 文件**

```powershell
Get-ChildItem -Path d:\projects\coder-micro-services\infrastructure -Recurse -Filter docker-compose*.yaml
```

- [ ] **Step 2: 加 depends_on（token-service → user-service + mysql）**

打开相关 compose 文件，定位 token-service service 段，加：
```yaml
    depends_on:
      - mysql
      - user-service
```
若 user-service 段没有 `depends_on: [mysql]`，也补上。

- [ ] **Step 3: commit**

```powershell
git add infrastructure
git commit -m "chore(infra): token-service depends_on user-service + mysql"
```

---

## Task 21: 最终验证

- [ ] **Step 1: fmt + clippy**

```powershell
cd d:\projects\coder-micro-services
cargo fmt --check
cargo clippy --all-targets -p token-service -- -D warnings
```
预期：无 warning / error。

- [ ] **Step 2: 跑 token-service 全量测试**

```powershell
cargo test -p token-service -- --test-threads=1
```
预期：所有 test PASS（oauth_client / auth_code / revoke / refresh_marker / integration + lib unit tests）。

- [ ] **Step 3: 跑项目全量 orchestrator**

```powershell
.\scripts\test-orchestrator.ps1 loop
```
预期：所有 service 测试通过；token-service 跑实 MySQL 容器。

- [ ] **Step 4: 启动 token-service 本地（手动 smoke）**

```powershell
$env:DATABASE_URL = "mysql://root:root@127.0.0.1:3306/token_service"
mysql -h 127.0.0.1 -u root -proot -e "CREATE DATABASE IF NOT EXISTS token_service"
cargo run -p token-service
```
预期：日志输出 `seeded 4 default oauth clients`、`connected to mysql`、`token-service listening on 0.0.0.0:4000`。

- [ ] **Step 5: 手 curl smoke**

```powershell
curl http://localhost:4000/health
curl -X POST http://localhost:4000/api/tokens/access -H "Content-Type: application/json" -d '{"grant_type":"authorization_code","code":"x","client_id":"api-gateway","client_secret":"dev-gateway-secret"}'
```
预期：`/health` 200；`/api/tokens/access` 因为 code 不存在返 400 `invalid or expired code`（说明 client_secret 验证通过）。

- [ ] **Step 6: 写完工报告**

向用户报告：
- 22 commits（Task 1, 4-20 各一个；Task 2 跟 Task 4 合并；Task 3 跟 Task 8 合并；Task 21 不 commit）
- test 数：4 + 5 + 4 + 3 + 1 = 17 集成 + lib 单元
- clippy clean / fmt clean
- docker-compose 启动顺序已补
