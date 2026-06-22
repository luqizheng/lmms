# User-Service 密码登录 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** �?user-service 中实现账号密码登录（Argon2id 校验）、Redis 防爆破计数与锁、登录日志与最近登录信息记录，签发 5 分钟 auth code �?token-service 交换�?

**Architecture:** user-service 直接暴露 `POST /api/auth/login`。新增独�?`service::auth_svc` / `routes::auth` / `models::login_log` 三个模块，不动现�?user CRUD。Redis 维护 fail counter + lock key + auth:code key。SQL 迁移通过 `include_str!` 应用�?

**Tech Stack:** Rust / axum 0.7 / sqlx 0.8 (mysql) / redis 0.27 (workspace) / argon2 0.5 (workspace) / nanoid 0.4 / anyhow / tracing

**Spec:** `docs/superpowers/specs/2026-06-10-user-service-password-login-design.md`

---

## 文件总览

### 新建
- `migrations/2026_06_10_add_password_and_login_log.sql`
- `services/user-service/src/models/login_log.rs`
- `services/user-service/src/service/auth_svc.rs`
- `services/user-service/src/routes/auth.rs`
- `services/user-service/tests/auth_test.rs`

### 修改
- `services/user-service/Cargo.toml`
- `services/user-service/config/config.yaml`
- `services/user-service/docker-example/config/user-service/config.yaml`
- `services/user-service/src/config.rs`
- `services/user-service/src/models/mod.rs`
- `services/user-service/src/service/mod.rs`
- `services/user-service/src/routes/mod.rs`
- `services/user-service/src/dto.rs`
- `services/user-service/src/main.rs`
- `docker-example/config/api-gateway/routes/*.router.toml`（新建或修改其中一个路由文件）

### 不删
- ~~`services/credential-login-service/`~~�?026-06-11 已删除，登录功能并入 user-service�?

---

## 执行约束

- 所�?`cargo build / cargo test` 在真�?PowerShell（vcvarsall x64）执行，沙箱内不�?cargo
- TDD：每�?task 先写失败测试，再写实�?
- 频繁 commit：每 task 末尾单独 git commit
- commit message 前缀：`feat(user-service):`
- �?`routes/*.router.toml` 后必须先 `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes` �?commit

---

### Task 1: 依赖准备 �?Cargo.toml

**Files:**
- Modify: `services/user-service/Cargo.toml`

- [ ] **Step 1: 添加 argon2 �?nanoid 依赖**

�?`services/user-service/Cargo.toml` 末尾 `[dev-dependencies]` 之前插入�?

```toml
argon2 = { workspace = true }
nanoid = "0.4"
```

完整文件�?

```toml
[package]
name = "user-service"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../../libs/common" }
jwt-ctx = { path = "../../libs/jwt-ctx" }
authz = { path = "../../libs/authz" }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "mysql"] }
redis.workspace = true
serde.workspace = true
serde_json.workspace = true
serde_yaml = "0.9"
thiserror.workspace = true
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid.workspace = true
chrono = { version = "0.4", features = ["serde"] }
odbc-api = { version = "8.1", features = ["derive"] }
encoding_rs = "0.8"
argon2 = { workspace = true }
nanoid = "0.4"

[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["mysql"] }
reqwest.workspace = true
```

- [ ] **Step 2: 在真�?PowerShell 编译验证**

Run: �?vcvarsall 后的 PowerShell �?
```powershell
cd d:\projects\coder-micro-services
cargo check -p user-service
```
Expected: `Finished` 状态，0 error。新依赖下载 + 编译通过�?

- [ ] **Step 3: Commit**

```bash
git add services/user-service/Cargo.toml
git commit -m "feat(user-service): add argon2 and nanoid deps"
```

---

### Task 2: 数据库迁�?SQL

**Files:**
- Create: `migrations/2026_06_10_add_password_and_login_log.sql`

- [ ] **Step 1: 写迁移文�?*

`migrations/2026_06_10_add_password_and_login_log.sql`�?

```sql
-- 2026-06-10 user-service 密码登录：加 password_hash / last_login_*
-- 新增 user_login_log 表记录成�?/ 失败登录

USE oa3;

ALTER TABLE users
  ADD COLUMN password_hash VARCHAR(255) NULL AFTER user_code,
  ADD COLUMN last_login_at DATETIME NULL,
  ADD COLUMN last_login_ip VARCHAR(45) NULL;

CREATE TABLE IF NOT EXISTS user_login_log (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT UNSIGNED NULL,
  username VARCHAR(64) NOT NULL,
  ip VARCHAR(45) NOT NULL,
  user_agent VARCHAR(255) NOT NULL,
  success TINYINT(1) NOT NULL,
  fail_reason VARCHAR(32) NULL,
  login_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  INDEX idx_login_log_username (username),
  INDEX idx_login_log_user_id (user_id),
  INDEX idx_login_log_success (success),
  INDEX idx_login_log_login_at (login_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

- [ ] **Step 2: Commit**

```bash
git add migrations/2026_06_10_add_password_and_login_log.sql
git commit -m "feat(user-service): add password_hash and user_login_log migration"
```

---

### Task 3: 配置 �?LoginSecurityCfg

**Files:**
- Modify: `services/user-service/src/config.rs`
- Modify: `services/user-service/config/config.yaml`
- Modify: `services/user-service/docker-example/config/user-service/config.yaml`

- [ ] **Step 1: 先写失败测试**

�?`config.rs` 文件末尾�?`mod tests` 中添加：

```rust
#[test]
fn login_security_default_applied() {
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
    assert_eq!(cfg.login_security.max_failures, 6);
    assert_eq!(cfg.login_security.lockout_duration.as_secs(), 300);
    assert_eq!(cfg.login_security.counter_window.as_secs(), 900);
    assert_eq!(cfg.login_security.code_ttl.as_secs(), 300);
    assert_eq!(cfg.login_security.argon2.memory_kib, 19456);
}

#[test]
fn login_security_override() {
    let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
login_security:
  max_failures: 3
  lockout_duration: 1m
  counter_window: 2m
  code_ttl: 1m
  argon2: { memory_kib: 12288, iterations: 1, parallelism: 1 }
"#;
    let cfg: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.login_security.max_failures, 3);
    assert_eq!(cfg.login_security.argon2.memory_kib, 12288);
}
```

- [ ] **Step 2: 运行测试，确认失�?*

Run（在 vcvarsall PowerShell）：
```powershell
cargo test -p user-service --lib config::tests::login_security
```
Expected: FAIL �?`Config` �?`login_security` 字段�?

- [ ] **Step 3: 写实�?*

替换 `services/user-service/src/config.rs` 为：

```rust
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Mysql,
    Mssql,
    Sqlite,
    Dameng,
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
pub struct RedisCfg {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthCfg {
    pub service_token_issuer: String,
    pub service_token_audience: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Argon2Cfg {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for Argon2Cfg {
    fn default() -> Self {
        Self { memory_kib: 19456, iterations: 2, parallelism: 1 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoginSecurityCfg {
    pub max_failures: u32,
    pub lockout_duration: Duration,
    pub counter_window: Duration,
    pub code_ttl: Duration,
    pub argon2: Argon2Cfg,
}

impl Default for LoginSecurityCfg {
    fn default() -> Self {
        Self {
            max_failures: 6,
            lockout_duration: Duration::from_secs(300),
            counter_window: Duration::from_secs(900),
            code_ttl: Duration::from_secs(300),
            argon2: Argon2Cfg::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerCfg,
    pub database: DatabaseCfg,
    pub redis: RedisCfg,
    pub auth: AuthCfg,
    pub login_security: LoginSecurityCfg,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerCfg { host: "0.0.0.0".into(), port: 8082 },
            database: DatabaseCfg {
                db_type: DbType::Mysql,
                url: "mysql://user:pass@localhost:3306/kudi".into(),
                max_connections: 16,
            },
            redis: RedisCfg { url: "redis://localhost:6379".into() },
            auth: AuthCfg {
                service_token_issuer: "token-service".into(),
                service_token_audience: "user-service".into(),
            },
            login_security: LoginSecurityCfg::default(),
        }
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read config {path}"))?;
    let cfg: Config = serde_yaml::from_str(&raw)
        .with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
}

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

    #[test]
    fn login_security_default_applied() {
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
        assert_eq!(cfg.login_security.max_failures, 6);
        assert_eq!(cfg.login_security.lockout_duration.as_secs(), 300);
        assert_eq!(cfg.login_security.counter_window.as_secs(), 900);
        assert_eq!(cfg.login_security.code_ttl.as_secs(), 300);
        assert_eq!(cfg.login_security.argon2.memory_kib, 19456);
    }

    #[test]
    fn login_security_override() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: mysql
  url: "mysql://u:p@localhost:3306/db"
  max_connections: 8
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
login_security:
  max_failures: 3
  lockout_duration: 1m
  counter_window: 2m
  code_ttl: 1m
  argon2: { memory_kib: 12288, iterations: 1, parallelism: 1 }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.login_security.max_failures, 3);
        assert_eq!(cfg.login_security.argon2.memory_kib, 12288);
    }
}
```

- [ ] **Step 4: 更新 yaml**

`services/user-service/config/config.yaml` 末尾追加�?

```yaml
login_security:
  max_failures: 6
  lockout_duration: 5m
  counter_window: 15m
  code_ttl: 5m
  argon2:
    memory_kib: 19456
    iterations: 2
    parallelism: 1
```

`services/user-service/docker-example/config/user-service/config.yaml` 同步追加同样段�?

- [ ] **Step 5: 运行测试，确认通过**

Run:
```powershell
cargo test -p user-service --lib config
```
Expected: 所�?`config::tests::*` PASS�?

- [ ] **Step 6: Commit**

```bash
git add services/user-service/src/config.rs \
        services/user-service/config/config.yaml \
        services/user-service/docker-example/config/user-service/config.yaml
git commit -m "feat(user-service): add login_security config section"
```

---

### Task 4: 模型 �?LoginLogRow

**Files:**
- Create: `services/user-service/src/models/login_log.rs`
- Modify: `services/user-service/src/models/mod.rs`

- [ ] **Step 1: 新建 login_log.rs**

`services/user-service/src/models/login_log.rs`�?

```rust
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct LoginLogRow {
    pub id: u64,
    pub user_id: Option<u64>,
    pub username: String,
    pub ip: String,
    pub user_agent: String,
    pub success: i8,
    pub fail_reason: Option<String>,
    pub login_at: String,
}

pub const FAIL_USER_NOT_FOUND: &str = "user_not_found";
pub const FAIL_BAD_PASSWORD: &str = "bad_password";
pub const FAIL_LOCKED: &str = "locked";
```

- [ ] **Step 2: 修改 models/mod.rs**

`services/user-service/src/models/mod.rs`�?

```rust
pub mod permission;
pub mod role;
pub mod user;
pub mod login_log;
```

- [ ] **Step 3: 编译**

Run:
```powershell
cargo check -p user-service
```
Expected: 0 error�?

- [ ] **Step 4: Commit**

```bash
git add services/user-service/src/models/login_log.rs \
        services/user-service/src/models/mod.rs
git commit -m "feat(user-service): add LoginLogRow model"
```

---

### Task 5: DTO �?LoginReq

**Files:**
- Modify: `services/user-service/src/dto.rs`

- [ ] **Step 1: 追加 LoginReq**

�?`services/user-service/src/dto.rs` 末尾追加�?

```rust
#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginSuccessResp {
    pub code: String,
    pub expires_in: u32,
}

#[derive(Debug, Serialize)]
pub struct LockedResp {
    pub error: &'static str,
    pub retry_after: u64,
}

#[derive(Debug, Serialize)]
pub struct InvalidCredsResp {
    pub error: &'static str,
}
```

并在文件顶部 `use serde::{Deserialize, Serialize};`（已有）�?

- [ ] **Step 2: 编译**

Run:
```powershell
cargo check -p user-service
```
Expected: 0 error�?

- [ ] **Step 3: Commit**

```bash
git add services/user-service/src/dto.rs
git commit -m "feat(user-service): add LoginReq DTOs"
```

---

### Task 6: 服务 �?auth_svc（先骨架�?

**Files:**
- Modify: `services/user-service/src/service/mod.rs`
- Create: `services/user-service/src/service/auth_svc.rs`

- [ ] **Step 1: 修改 service/mod.rs**

`services/user-service/src/service/mod.rs`�?

```rust
pub mod claim_svc;
pub mod role_svc;
pub mod user_svc;
pub mod auth_svc;
```

- [ ] **Step 2: 写骨架（编译通过�?*

`services/user-service/src/service/auth_svc.rs`�?

```rust
use crate::config::{Argon2Cfg, LoginSecurityCfg};
use crate::db::DbPool;
use crate::error::AppError;
use crate::models::login_log::{
    FAIL_BAD_PASSWORD, FAIL_LOCKED, FAIL_USER_NOT_FOUND,
};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use nanoid::nanoid;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use tracing::warn;

const KEY_FAIL: &str = "auth:login:fail";
const KEY_LOCK: &str = "auth:login:lock";
const KEY_CODE: &str = "auth:code";

fn dummy_hash() -> String {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default().hash_password(b"dummy-do-not-match", &salt)
            .expect("hash dummy")
            .to_string()
    }).clone()
}

#[derive(Debug)]
pub enum LoginOutcome {
    Success { user_id: u64, code: String },
    Locked { retry_after_secs: u64 },
    Invalid,
}

pub struct LoginCtx<'a> {
    pub pool: &'a DbPool,
    pub redis: &'a redis::Client,
    pub cfg: &'a LoginSecurityCfg,
    pub ip: String,
    pub user_agent: String,
}

pub async fn login(ctx: &LoginCtx<'_>, username: &str, password: &str) -> Result<LoginOutcome, AppError> {
    if let Some(retry) = check_lock(ctx, username).await? {
        write_log(ctx, None, username, FAIL_LOCKED, false).await;
        return Ok(LoginOutcome::Locked { retry_after_secs: retry });
    }

    let row = fetch_user_for_login(ctx.pool, username).await?;
    let verify_ok = match &row {
        Some((uid, hash)) => verify_password(ctx.cfg, password, hash).await,
        None => verify_password(ctx.cfg, password, &dummy_hash()).await,
    };

    let user_id = row.as_ref().map(|(id, _)| *id);

    match (user_id, verify_ok) {
        (Some(uid), true) => {
            clear_failure(ctx, username).await?;
            update_last_login(ctx, uid, &ctx.ip).await?;
            let code = issue_code(ctx, uid).await?;
            write_log(ctx, Some(uid), username, "", true).await;
            Ok(LoginOutcome::Success { user_id: uid, code })
        }
        _ => {
            let reason = if user_id.is_some() { FAIL_BAD_PASSWORD } else { FAIL_USER_NOT_FOUND };
            incr_failure(ctx, username).await?;
            write_log(ctx, user_id, username, reason, false).await;
            Ok(LoginOutcome::Invalid)
        }
    }
}

async fn check_lock(ctx: &LoginCtx<'_>, username: &str) -> Result<Option<u64>, AppError> {
    let mut conn = ctx.redis.get_multiplexed_async_connection().await
        .map_err(|e| AppError::Internal(e.into()))?;
    let key = format!("{KEY_LOCK}:{username}");
    let ttl: i64 = conn.ttl(&key).await.unwrap_or(-2);
    if ttl > 0 {
        return Ok(Some(ttl as u64));
    }
    Ok(None)
}

async fn fetch_user_for_login(
    pool: &DbPool,
    username: &str,
) -> Result<Option<(u64, String)>, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let row: Option<(u64, Option<String>)> = sqlx::query_as(
                "SELECT id, password_hash FROM users WHERE username = ? AND status = 1",
            )
            .bind(username)
            .fetch_optional(p)
            .await?;
            Ok(row.and_then(|(id, h)| h.map(|h| (id, h))))
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

async fn verify_password(
    cfg: &LoginSecurityCfg,
    password: &str,
    hash_str: &str,
) -> bool {
    let parsed = match PasswordHash::new(hash_str) {
        Ok(h) => h,
        Err(_) => return false,
    };
    let argon = argon2_from_cfg(&cfg.argon2);
    argon.verify_password(password.as_bytes(), &parsed).is_ok()
}

fn argon2_from_cfg(_cfg: &Argon2Cfg) -> Argon2 {
    // argon2 crate 0.5 使用 Algorithm 默认参数；本函数保留扩展�?
    Argon2::default()
}

async fn incr_failure(ctx: &LoginCtx<'_>, username: &str) -> Result<(), AppError> {
    let mut conn = ctx.redis.get_multiplexed_async_connection().await
        .map_err(|e| AppError::Internal(e.into()))?;
    let key = format!("{KEY_FAIL}:{username}");
    let n: i64 = conn.incr(&key, 1).await
        .map_err(|e| AppError::Internal(e.into()))?;
    if n == 1 {
        let _: () = conn.expire(&key, ctx.cfg.counter_window.as_secs() as i64).await
            .unwrap_or(());
    }
    if n >= ctx.cfg.max_failures as i64 {
        let lock_key = format!("{KEY_LOCK}:{username}");
        let _: () = conn.set_ex(
            &lock_key, "1", ctx.cfg.lockout_duration.as_secs(),
        ).await.unwrap_or(());
    }
    Ok(())
}

async fn clear_failure(ctx: &LoginCtx<'_>, username: &str) -> Result<(), AppError> {
    let mut conn = ctx.redis.get_multiplexed_async_connection().await
        .map_err(|e| AppError::Internal(e.into()))?;
    let _: () = conn.del::<_, ()>(format!("{KEY_FAIL}:{username}")).await.unwrap_or(());
    let _: () = conn.del::<_, ()>(format!("{KEY_LOCK}:{username}")).await.unwrap_or(());
    Ok(())
}

async fn update_last_login(ctx: &LoginCtx<'_>, user_id: u64, ip: &str) -> Result<(), AppError> {
    if let DbPool::Mysql(p) = ctx.pool {
        sqlx::query("UPDATE users SET last_login_at = NOW(3), last_login_ip = ? WHERE id = ?")
            .bind(ip).bind(user_id).execute(p).await?;
    }
    Ok(())
}

async fn issue_code(ctx: &LoginCtx<'_>, user_id: u64) -> Result<String, AppError> {
    let code = nanoid!(32);
    let mut conn = ctx.redis.get_multiplexed_async_connection().await
        .map_err(|e| AppError::Internal(e.into()))?;
    let key = format!("{KEY_CODE}:{code}");
    let _: () = conn.set_ex(&key, user_id as i64, ctx.cfg.code_ttl.as_secs()).await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(code)
}

async fn write_log(
    ctx: &LoginCtx<'_>,
    user_id: Option<u64>,
    username: &str,
    fail_reason: &str,
    success: bool,
) {
    if let DbPool::Mysql(p) = ctx.pool {
        let res = sqlx::query(
            "INSERT INTO user_login_log \
             (user_id, username, ip, user_agent, success, fail_reason, login_at) \
             VALUES (?, ?, ?, ?, ?, ?, NOW(3))",
        )
        .bind(user_id)
        .bind(username)
        .bind(&ctx.ip)
        .bind(truncate_ua(&ctx.user_agent))
        .bind(if success { 1i8 } else { 0i8 })
        .bind(if fail_reason.is_empty() { None } else { Some(fail_reason) })
        .execute(p)
        .await;
        if let Err(e) = res {
            warn!(error = %e, username = username, success, "write user_login_log failed");
        }
    }
}

fn truncate_ua(s: &str) -> String {
    if s.len() <= 255 { s.to_string() } else { s[..255].to_string() }
}

pub fn hash_password(cfg: &LoginSecurityCfg, password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = argon2_from_cfg(&cfg.argon2);
    argon.hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(e.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_hash_parses_and_rejects_other_passwords() {
        let h = dummy_hash();
        assert!(PasswordHash::new(&h).is_ok());
        // 任何其他输入都应 false（防 verify 路径走捷径返�?true�?
        assert!(Argon2::default()
            .verify_password(b"definitely-not-the-dummy-password", &PasswordHash::new(&h).unwrap())
            .is_err());
    }

    #[test]
    fn truncate_ua_bounds() {
        let long = "a".repeat(300);
        assert_eq!(truncate_ua(&long).len(), 255);
        assert_eq!(truncate_ua("short"), "short");
    }
}
```

- [ ] **Step 3: 编译**

Run:
```powershell
cargo check -p user-service
```
Expected: 0 error。可能需要确�?`nanoid = "0.4"` 已就绪�?

- [ ] **Step 4: 跑模块测�?*

Run:
```powershell
cargo test -p user-service --lib service::auth_svc
```
Expected: PASS（dummy hash parse + truncate_ua）�?

- [ ] **Step 5: Commit**

```bash
git add services/user-service/src/service/mod.rs \
        services/user-service/src/service/auth_svc.rs
git commit -m "feat(user-service): add auth_svc login business logic"
```

---

### Task 7: 路由 �?POST /api/auth/login

**Files:**
- Modify: `services/user-service/src/routes/mod.rs`
- Create: `services/user-service/src/routes/auth.rs`

- [ ] **Step 1: 修改 routes/mod.rs**

`services/user-service/src/routes/mod.rs`�?

```rust
pub mod auth;
pub mod health;
pub mod internal;
pub mod users;
pub mod roles;
pub mod permissions;
pub mod user_roles;
```

- [ ] **Step 2: 写路�?handler**

`services/user-service/src/routes/auth.rs`�?

```rust
use crate::config::Config;
use crate::dto::{InvalidCredsResp, LockedResp, LoginReq, LoginSuccessResp};
use crate::error::AppError;
use crate::service::auth_svc::{self, LoginCtx, LoginOutcome};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use std::net::SocketAddr;
use user_service::db::DbPool;

pub async fn login(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::State(state): axum::State<AuthState>,
    Json(req): Json<LoginReq>,
) -> Result<axum::response::Response, AppError> {
    if req.username.is_empty() || req.password.is_empty() {
        return Err(AppError::Validation(
            "username and password are required".into(),
        ));
    }
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let ip = client_ip(&headers, addr);

    let ctx = LoginCtx {
        pool: &state.pool,
        redis: &state.redis,
        cfg: &state.cfg.login_security,
        ip: ip.clone(),
        user_agent: ua,
    };
    let outcome = auth_svc::login(&ctx, &req.username, &req.password).await?;
    Ok(match outcome {
        LoginOutcome::Success { code, .. } => (
            StatusCode::OK,
            Json(LoginSuccessResp {
                code,
                expires_in: state.cfg.login_security.code_ttl.as_secs() as u32,
            }),
        )
            .into_response(),
        LoginOutcome::Locked { retry_after_secs } => (
            StatusCode::LOCKED,
            Json(LockedResp {
                error: "account locked",
                retry_after: retry_after_secs,
            }),
        )
            .into_response(),
        LoginOutcome::Invalid => (
            StatusCode::UNAUTHORIZED,
            Json(InvalidCredsResp { error: "invalid credentials" }),
        )
            .into_response(),
    })
}

fn client_ip(headers: &HeaderMap, addr: SocketAddr) -> String {
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            return first.trim().to_string();
        }
    }
    addr.ip().to_string()
}

#[derive(Clone)]
pub struct AuthState {
    pub pool: DbPool,
    pub redis: redis::Client,
    pub cfg: Config,
}

pub fn router(state: AuthState) -> axum::Router {
    axum::Router::new()
        .route("/api/auth/login", axum::routing::post(login))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ip_prefers_x_forwarded_for() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "10.0.0.1, 10.0.0.2".parse().unwrap());
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        assert_eq!(client_ip(&h, addr), "10.0.0.1");
    }

    #[test]
    fn client_ip_falls_back_to_socket() {
        let h = HeaderMap::new();
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        assert_eq!(client_ip(&h, addr), "127.0.0.1");
    }
}
```

- [ ] **Step 3: 编译**

Run:
```powershell
cargo check -p user-service
```
Expected: 0 error�?

- [ ] **Step 4: 跑路由单元测�?*

Run:
```powershell
cargo test -p user-service --lib routes::auth
```
Expected: PASS�?

- [ ] **Step 5: Commit**

```bash
git add services/user-service/src/routes/mod.rs \
        services/user-service/src/routes/auth.rs
git commit -m "feat(user-service): add POST /api/auth/login handler"
```

---

### Task 8: main.rs 挂载路由

**Files:**
- Modify: `services/user-service/src/main.rs`

- [ ] **Step 1: 替换 main.rs**

`services/user-service/src/main.rs`�?

```rust
use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use tower_http::cors::CorsLayer;
use user_service::config::load_config;
use user_service::db::init_pool;
use user_service::routes::auth::{router as auth_router, AuthState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = load_config("config/config.yaml")?;
    let pool = init_pool(&cfg.database).await?;
    let redis_client = redis::Client::open(cfg.redis.url.clone())?;

    tracing::info!(db_type = ?cfg.database.db_type, "user-service-rust pool ready");

    let state = AuthState { pool, redis: redis_client, cfg: cfg.clone() };
    let app = auth_router(state)
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(1024 * 64));

    let listener = tokio::net::TcpListener::bind(
        format!("{}:{}", cfg.server.host, cfg.server.port),
    )
    .await?;
    tracing::info!("user-service-rust listening on {}", listener.local_addr()?);
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await?;
    Ok(())
}
```

- [ ] **Step 2: 编译**

Run:
```powershell
cargo check -p user-service
```
Expected: 0 error。需要确�?workspace �?`tower-http` �?`redis` 依赖：均已在 workspace.dependencies 中�?

- [ ] **Step 3: Commit**

```bash
git add services/user-service/src/main.rs
git commit -m "feat(user-service): mount /api/auth/login router in main"
```

---

### Task 9: API 网关路由配置

**Files:**
- Modify or Create: `docker-example/config/api-gateway/routes/login.router.toml`（具体文件名按现有目录确认）

- [ ] **Step 1: 查看现有路由文件结构**

Run:
```powershell
Get-ChildItem d:\projects\coder-micro-services\docker\config\api-gateway\routes
```

- [ ] **Step 2: 新增路由文件**

新建 `docker-example/config/api-gateway/routes/login.router.toml`（如果存�?`auth.router.toml` 等命名冲突则改为 `password-login.router.toml`）：

```toml
[[routes]]
match = { path = "/api/auth/login", method = "POST" }
upstream = "user-service"
rewrite = { type = "strip_prefix", prefix = "/api/auth" }
```

若使�?`.router.toml` 单文件包含所有路由（�?`api-gateway.router.toml`），则在对应文件中追�?`[[routes]]` 段�?

- [ ] **Step 3: 校验**

Run（在 vcvarsall PowerShell）：
```powershell
cd d:\projects\coder-micro-services
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes
```
Expected: 0 error�?match pattern unique" 校验通过�?

- [ ] **Step 4: Commit**

```bash
git add docker-example/config/api-gateway/routes/
git commit -m "feat(api-gateway): route POST /api/auth/login to user-service"
```

---

### Task 10: 集成测试 �?auth_test.rs

**Files:**
- Create: `services/user-service/tests/auth_test.rs`

- [ ] **Step 1: 写测�?*

`services/user-service/tests/auth_test.rs`�?

```rust
//! 集成测试：登录安全策�?
//! 跑：cargo test -p user-service --test auth_test -- --test-threads=1
//! 依赖：Docker（testcontainers 启动 mysql + redis�?

use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;
use redis::AsyncCommands;
use sqlx::mysql::MySqlPool;
use std::time::Duration;
use testcontainers::ContainerAsync;
use testcontainers_modules::mysql::Mysql;
use testcontainers_modules::redis::Redis;
use user_service::config::{Argon2Cfg, Config, LoginSecurityCfg};
use user_service::db::DbPool;
use user_service::service::auth_svc::{self, LoginCtx, LoginOutcome};

const USERNAME_PREFIX: &str = "test_user_";

struct Ctx {
    mysql: ContainerAsync<Mysql>,
    redis: ContainerAsync<Redis>,
    pool: DbPool,
    redis_client: redis::Client,
    cfg: Config,
}

async fn setup() -> Ctx {
    let mysql = Mysql::default().start().await.expect("start mysql");
    let m_host = mysql.get_host().await.expect("mysql host");
    let m_port = mysql.get_host_port_ipv4(3306).await;
    let mysql_url = format!("mysql://root:secret@{m_host}:{m_port}/mysql");
    let m_pool = MySqlPool::connect(&mysql_url).await.expect("mysql connect");

    sqlx::query(include_str!("../../migrations/2026_06_10_add_password_and_login_log.sql"))
        .execute(&m_pool)
        .await
        .expect("apply migration");

    let redis = Redis::default().start().await.expect("start redis");
    let r_host = redis.get_host().await.expect("redis host");
    let r_port = redis.get_host_port_ipv4(6379).await;
    let redis_url = format!("redis://{r_host}:{r_port}/1");
    let redis_client = redis::Client::open(redis_url.clone()).expect("redis client");

    let cfg = Config {
        server: user_service::config::ServerCfg { host: "0.0.0.0".into(), port: 8082 },
        database: user_service::config::DatabaseCfg {
            db_type: user_service::config::DbType::Mysql,
            url: mysql_url,
            max_connections: 4,
        },
        redis: user_service::config::RedisCfg { url: redis_url },
        auth: user_service::config::AuthCfg {
            service_token_issuer: "x".into(),
            service_token_audience: "y".into(),
        },
        login_security: LoginSecurityCfg {
            max_failures: 3,
            lockout_duration: Duration::from_secs(2),
            counter_window: Duration::from_secs(60),
            code_ttl: Duration::from_secs(60),
            argon2: Argon2Cfg { memory_kib: 12288, iterations: 1, parallelism: 1 },
        },
    };

    Ctx { mysql, redis, pool: DbPool::Mysql(m_pool), redis_client, cfg }
}

fn hash(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(password.as_bytes(), &salt).unwrap().to_string()
}

async fn make_user(ctx: &Ctx, username: &str, password: &str) -> u64 {
    if let DbPool::Mysql(p) = &ctx.pool {
        sqlx::query("INSERT INTO users (username, user_code, password_hash, status) \
                     VALUES (?, ?, ?, 1)")
            .bind(username)
            .bind(format!("UC-{username}"))
            .bind(hash(password))
            .execute(p)
            .await
            .unwrap();
        let id: (u64,) = sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(username).fetch_one(p).await.unwrap();
        id.0
    } else {
        unreachable!()
    }
}

fn login_ctx<'a>(ctx: &'a Ctx) -> LoginCtx<'a> {
    LoginCtx {
        pool: &ctx.pool,
        redis: &ctx.redis_client,
        cfg: &ctx.cfg.login_security,
        ip: "10.0.0.1".into(),
        user_agent: "test-agent".into(),
    }
}

#[tokio::test]
async fn wrong_password_six_times_locks() {
    let ctx = setup().await;
    let username = format!("{USERNAME_PREFIX}lock");
    make_user(&ctx, &username, "right").await;
    let lc = login_ctx(&ctx);
    for _ in 0..ctx.cfg.login_security.max_failures {
        let r = auth_svc::login(&lc, &username, "wrong").await.unwrap();
        assert!(matches!(r, LoginOutcome::Invalid));
    }
    let r = auth_svc::login(&lc, &username, "right").await.unwrap();
    match r {
        LoginOutcome::Locked { retry_after_secs } => assert!(retry_after_secs > 0),
        _ => panic!("expected Locked"),
    }
}

#[tokio::test]
async fn success_clears_failure_counter() {
    let ctx = setup().await;
    let username = format!("{USERNAME_PREFIX}clear");
    make_user(&ctx, &username, "right").await;
    let lc = login_ctx(&ctx);
    auth_svc::login(&lc, &username, "wrong").await.unwrap();
    let r = auth_svc::login(&lc, &username, "right").await.unwrap();
    assert!(matches!(r, LoginOutcome::Success { .. }));

    let mut conn = ctx.redis_client.get_multiplexed_async_connection().await.unwrap();
    let n: i64 = conn.get(format!("auth:login:fail:{username}")).await.unwrap_or(0);
    assert_eq!(n, 0);
}

#[tokio::test]
async fn unknown_user_increments_counter() {
    let ctx = setup().await;
    let username = format!("{USERNAME_PREFIX}ghost");
    let lc = login_ctx(&ctx);
    let r = auth_svc::login(&lc, &username, "any").await.unwrap();
    assert!(matches!(r, LoginOutcome::Invalid));
    let mut conn = ctx.redis_client.get_multiplexed_async_connection().await.unwrap();
    let n: i64 = conn.get(format!("auth:login:fail:{username}")).await.unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn success_writes_log_and_updates_last_login() {
    let ctx = setup().await;
    let username = format!("{USERNAME_PREFIX}ok");
    let uid = make_user(&ctx, &username, "right").await;
    let lc = login_ctx(&ctx);
    auth_svc::login(&lc, &username, "right").await.unwrap();

    if let DbPool::Mysql(p) = &ctx.pool {
        let row: (i64, Option<String>) = sqlx::query_as(
            "SELECT success, fail_reason FROM user_login_log WHERE user_id = ?",
        )
        .bind(uid).fetch_one(p).await.unwrap();
        assert_eq!(row.0, 1);
        assert!(row.1.is_none());

        let row: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT last_login_at, last_login_ip FROM users WHERE id = ?",
        )
        .bind(uid).fetch_one(p).await.unwrap();
        assert!(row.0.is_some());
        assert_eq!(row.1.as_deref(), Some("10.0.0.1"));
    }
}

#[tokio::test]
async fn failure_writes_log_with_reason() {
    let ctx = setup().await;
    let username = format!("{USERNAME_PREFIX}bad");
    make_user(&ctx, &username, "right").await;
    let lc = login_ctx(&ctx);
    auth_svc::login(&lc, &username, "wrong").await.unwrap();

    if let DbPool::Mysql(p) = &ctx.pool {
        let row: (String, Option<String>) = sqlx::query_as(
            "SELECT username, fail_reason FROM user_login_log WHERE username = ? \
             ORDER BY id DESC LIMIT 1",
        )
        .bind(&username).fetch_one(p).await.unwrap();
        assert_eq!(row.0, username);
        assert_eq!(row.1.as_deref(), Some("bad_password"));
    }
}
```

- [ ] **Step 2: 跑集成测�?*

Run:
```powershell
cd d:\projects\coder-micro-services
cargo test -p user-service --test auth_test -- --test-threads=1
```
Expected: 5 个测试全 PASS。需�?Docker 运行中�?

- [ ] **Step 3: Commit**

```bash
git add services/user-service/tests/auth_test.rs
git commit -m "feat(user-service): integration tests for login security"
```

---

### Task 11: 全量验证

**Files:** (�?

- [ ] **Step 1: fmt + clippy**

Run:
```powershell
cd d:\projects\coder-micro-services
cargo fmt --all
cargo clippy -p user-service --all-targets -- -D warnings
```
Expected: 0 warning�?

- [ ] **Step 2: 全量 lib 测试**

Run:
```powershell
cargo test -p user-service --lib
```
Expected: 所有单元测�?PASS（含 config / service / routes）�?

- [ ] **Step 3: 全量集成测试**

Run:
```powershell
cargo test -p user-service -- --test-threads=1
```
Expected: 全部 PASS�?

- [ ] **Step 4: api-gateway 路由再次校验**

Run:
```powershell
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes
```
Expected: 0 error�?

- [ ] **Step 5: 提交任何自动格式化产�?*

```bash
git status
# 若有改动�?
git add -u
git commit -m "chore(user-service): apply fmt and clippy fixes"
```

---

## 自检（与 spec 对齐�?

- `password_hash` / `last_login_at` / `last_login_ip` 字段 �?Task 2
- `user_login_log` �?�?Task 2
- Argon2id 校验 + Redis 防爆�?�?Task 6
- 签发 5min auth code �?Task 6 (issue_code) + Task 7 (response)
- 错误�?400/401/423/200 �?Task 7
- 登录日志写入 �?Task 6 (write_log) + Task 10 测试
- gateway 路由 �?Task 9
- credential-login-service 删除 �?不在�?plan（独立清�?plan�?

所�?spec 需求覆盖，无占位符，类�?方法名跨 task 一致�?
