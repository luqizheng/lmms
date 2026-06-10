# 端到端登录 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 frontend → api-gateway → user-service + token-service 完整登录链路，账号 `admin/admin123` 可成功登录。

**Architecture:** 沿用 `user-service-password-login-plan` 后端登录，新增 `register`/`setup-status` 端点；首位注册 = admin。前端 Vite dev proxy 联调。Gateway 新增 `auth.router.toml` 替代 stub。`token-service` 启动时用 dev 明文现算 argon2 哈希。

**Tech Stack:** Rust / axum 0.7 / sqlx 0.8 / redis 0.27 / argon2 0.5 / nanoid 0.4 + Vue 3 / Vite / pinia / axios

**Spec:** `docs/superpowers/specs/2026-06-11-frontend-e2e-login-design.md`
**上游计划:** `docs/superpowers/plans/2026-06-10-user-service-password-login-plan.md`

---

## 文件总览

### 新建
- `migrations/2026_06_11_auth_login_and_register.sql`
- `docker/config/api-gateway/routes/auth.router.toml`
- `frontend/src/views/login/register.vue`
- `docs/superpowers/specs/2026-06-11-frontend-e2e-login-design.md`（已建）

### 修改
- `services/user-service/Cargo.toml`（+argon2, +nanoid）
- `services/user-service/src/dto.rs`（+RegisterReq/Resp）
- `services/user-service/src/config.rs`（+LoginSecurityCfg/Argon2Cfg）
- `services/user-service/src/models/login_log.rs`
- `services/user-service/src/models/mod.rs`
- `services/user-service/src/service/auth_svc.rs`
- `services/user-service/src/service/mod.rs`
- `services/user-service/src/routes/auth.rs`
- `services/user-service/src/routes/mod.rs`
- `services/user-service/src/main.rs`
- `services/user-service/config/config.yaml`（+login_security）
- `services/user-service/docker/config/user-service/config.yaml`（+login_security）
- `services/token-service/src/store/client.rs`（启动时明文→哈希）
- `docker/config/api-gateway/routes/credential-login.router.toml`（删）
- `frontend/vite.config.ts`（+proxy）
- `frontend/src/api/auth.ts`
- `frontend/src/api/token.ts`
- `frontend/src/api/user.ts`
- `frontend/src/utils/http/index.ts`（白名单）
- `frontend/src/views/login/index.vue`
- `frontend/src/router/modules/account.ts`（+register）
- `frontend/src/router/index.ts`（白名单）
- `frontend/.env.development`（+VITE_CLIENT_SECRET）

---

## 执行约束

- 所有 `cargo build / cargo test` 在真实 PowerShell（vcvarsall x64）执行，沙箱内不跑 cargo
- TDD：每个 task 先写失败测试，再写实现
- 频繁 commit：每 task 末尾单独 git commit
- commit message 前缀：`feat(user-service):` / `feat(token-service):` / `feat(api-gateway):` / `feat(frontend):`
- 改 `routes/*.router.toml` 后必须先 `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes` 再 commit

---

### Task 1: user-service 依赖 — Cargo.toml

**Files:**
- Modify: `services/user-service/Cargo.toml`

- [ ] **Step 1: 在真实 PowerShell 编辑 Cargo.toml**

`services/user-service/Cargo.toml` 末尾 `[dev-dependencies]` 之前插入：

```toml
argon2 = { workspace = true }
nanoid = "0.4"
```

最终依赖块：

```toml
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
testcontainers-modules = { version = "0.8", features = ["mysql", "redis"] }
reqwest.workspace = true
```

- [ ] **Step 2: 编译验证**

Run（在 vcvarsall x64 PowerShell）：
```powershell
cd d:\projects\coder-micro-services
cargo check -p user-service
```
Expected: 0 error。

- [ ] **Step 3: Commit**

```bash
git add services/user-service/Cargo.toml
git commit -m "feat(user-service): add argon2 and nanoid deps"
```

---

### Task 2: 数据库迁移 SQL

**Files:**
- Create: `migrations/2026_06_11_auth_login_and_register.sql`

- [ ] **Step 1: 新建迁移文件**

`migrations/2026_06_11_auth_login_and_register.sql`：

```sql
USE oa3;

-- 沿用 password-login-plan：加 password_hash / last_login_*
ALTER TABLE users
  ADD COLUMN password_hash VARCHAR(255) NULL AFTER user_code,
  ADD COLUMN last_login_at DATETIME NULL,
  ADD COLUMN last_login_ip VARCHAR(45) NULL;

-- 登录日志
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

-- 预置角色（幂等）
INSERT IGNORE INTO roles (code, name, status) VALUES
  ('admin', '系统管理员', 1),
  ('user',  '普通用户',   1);
```

- [ ] **Step 2: Commit**

```bash
git add migrations/2026_06_11_auth_login_and_register.sql
git commit -m "feat(user-service): migration for login + first-admin role seed"
```

---

### Task 3: 配置 — LoginSecurityCfg

**Files:**
- Modify: `services/user-service/src/config.rs`
- Modify: `services/user-service/config/config.yaml`
- Modify: `services/user-service/docker/config/user-service/config.yaml`

- [ ] **Step 1: 先写失败测试**

在 `services/user-service/src/config.rs` 文件末尾 `mod tests` 中追加：

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

- [ ] **Step 2: 运行测试，确认失败**

```powershell
cargo test -p user-service --lib config::tests::login_security
```
Expected: FAIL（无 `login_security` 字段）。

- [ ] **Step 3: 写实现**

`services/user-service/src/config.rs` 替换为：

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
pub struct ServerCfg { pub host: String, pub port: u16 }

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseCfg {
    pub db_type: DbType,
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisCfg { pub url: String }

#[derive(Debug, Clone, Deserialize)]
pub struct AuthCfg {
    pub service_token_issuer: String,
    pub service_token_audience: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct Argon2Cfg {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for Argon2Cfg {
    fn default() -> Self { Self { memory_kib: 19456, iterations: 2, parallelism: 1 } }
}

#[derive(Debug, Clone, Copy, Deserialize)]
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
    let raw = fs::read_to_string(path).with_context(|| format!("read config {path}"))?;
    let cfg: Config = serde_yaml::from_str(&raw).with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

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

`services/user-service/config/config.yaml` 末尾追加：

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

`services/user-service/docker/config/user-service/config.yaml` 同步追加同样段。

- [ ] **Step 5: 跑测试确认通过**

```powershell
cargo test -p user-service --lib config
```
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
git add services/user-service/src/config.rs \
        services/user-service/config/config.yaml \
        services/user-service/docker/config/user-service/config.yaml
git commit -m "feat(user-service): add login_security config section"
```

---

### Task 4: 模型 — LoginLogRow

**Files:**
- Create: `services/user-service/src/models/login_log.rs`
- Modify: `services/user-service/src/models/mod.rs`

- [ ] **Step 1: 新建 login_log.rs**

`services/user-service/src/models/login_log.rs`：

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

`services/user-service/src/models/mod.rs`：

```rust
pub mod permission;
pub mod role;
pub mod user;
pub mod login_log;
```

- [ ] **Step 3: 编译**

```powershell
cargo check -p user-service
```
Expected: 0 error。

- [ ] **Step 4: Commit**

```bash
git add services/user-service/src/models/login_log.rs \
        services/user-service/src/models/mod.rs
git commit -m "feat(user-service): add LoginLogRow model"
```

---

### Task 5: DTO — Login & Register

**Files:**
- Modify: `services/user-service/src/dto.rs`

- [ ] **Step 1: 追加 DTO**

`services/user-service/src/dto.rs` 末尾追加：

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

#[derive(Debug, Deserialize)]
pub struct RegisterReq {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResp {
    pub id: u64,
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct SetupStatusResp {
    pub setup_required: bool,
}
```

确保文件顶部有 `use serde::{Deserialize, Serialize};`。

- [ ] **Step 2: 编译**

```powershell
cargo check -p user-service
```
Expected: 0 error。

- [ ] **Step 3: Commit**

```bash
git add services/user-service/src/dto.rs
git commit -m "feat(user-service): add Login/Register DTOs"
```

---

### Task 6: 服务 — auth_svc（login + register + setup_status）

**Files:**
- Modify: `services/user-service/src/service/mod.rs`
- Create: `services/user-service/src/service/auth_svc.rs`

- [ ] **Step 1: 修改 service/mod.rs**

`services/user-service/src/service/mod.rs`：

```rust
pub mod claim_svc;
pub mod role_svc;
pub mod user_svc;
pub mod auth_svc;
```

- [ ] **Step 2: 写 auth_svc.rs**

`services/user-service/src/service/auth_svc.rs`：

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
use sqlx::Row;
use std::time::Duration;
use tracing::warn;

const KEY_FAIL: &str = "auth:login:fail";
const KEY_LOCK: &str = "auth:login:lock";
const KEY_CODE: &str = "auth:code";

fn dummy_hash() -> String {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default().hash_password(b"dummy-do-not-match", &salt)
            .expect("hash dummy").to_string()
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

pub async fn login(
    ctx: &LoginCtx<'_>, username: &str, password: &str,
) -> Result<LoginOutcome, AppError> {
    if let Some(retry) = check_lock(ctx, username).await? {
        write_log(ctx, None, username, FAIL_LOCKED, false).await;
        return Ok(LoginOutcome::Locked { retry_after_secs: retry });
    }

    let row = fetch_user_for_login(ctx.pool, username).await?;
    let verify_ok = match &row {
        Some((_, hash)) => verify_password(ctx.cfg, password, hash).await,
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
    if ttl > 0 { Ok(Some(ttl as u64)) } else { Ok(None) }
}

async fn fetch_user_for_login(
    pool: &DbPool, username: &str,
) -> Result<Option<(u64, String)>, AppError> {
    if let DbPool::Mysql(p) = pool {
        let row: Option<(u64, Option<String>)> = sqlx::query_as(
            "SELECT id, password_hash FROM users WHERE username = ? AND status = 1",
        )
        .bind(username).fetch_optional(p).await?;
        Ok(row.and_then(|(id, h)| h.map(|h| (id, h))))
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

async fn verify_password(cfg: &LoginSecurityCfg, password: &str, hash_str: &str) -> bool {
    let parsed = match PasswordHash::new(hash_str) { Ok(h) => h, Err(_) => return false };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

async fn incr_failure(ctx: &LoginCtx<'_>, username: &str) -> Result<(), AppError> {
    let mut conn = ctx.redis.get_multiplexed_async_connection().await
        .map_err(|e| AppError::Internal(e.into()))?;
    let key = format!("{KEY_FAIL}:{username}");
    let n: i64 = conn.incr(&key, 1).await.map_err(|e| AppError::Internal(e.into()))?;
    if n == 1 {
        let _: () = conn.expire(&key, ctx.cfg.counter_window.as_secs() as i64).await.unwrap_or(());
    }
    if n >= ctx.cfg.max_failures as i64 {
        let lock_key = format!("{KEY_LOCK}:{username}");
        let _: () = conn.set_ex(&lock_key, "1", ctx.cfg.lockout_duration.as_secs())
            .await.unwrap_or(());
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
    Argon2::default().hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(e.into()))
}

#[derive(Debug)]
pub enum RegisterError {
    Closed,
    UsernameTaken,
    InvalidInput(String),
    Internal(String),
}

pub async fn setup_status(pool: &DbPool) -> Result<bool, AppError> {
    if let DbPool::Mysql(p) = pool {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM users WHERE status = 1")
            .fetch_one(p).await?;
        let c: i64 = row.try_get("c").unwrap_or(0);
        Ok(c == 0)
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn register_admin(
    pool: &DbPool,
    cfg: &LoginSecurityCfg,
    username: &str,
    password: &str,
    email: Option<String>,
    phone: Option<String>,
    nickname: Option<String>,
) -> Result<u64, RegisterError> {
    if !is_valid_username(username) {
        return Err(RegisterError::InvalidInput("username must be 3-32 chars [a-zA-Z0-9_.-]".into()));
    }
    if password.len() < 6 {
        return Err(RegisterError::InvalidInput("password must be at least 6 chars".into()));
    }
    if let Some(e) = email.as_deref() {
        if !e.is_empty() && !e.contains('@') {
            return Err(RegisterError::InvalidInput("invalid email".into()));
        }
    }
    if let Some(p) = phone.as_deref() {
        if !p.is_empty() && (p.len() != 11 || !p.chars().all(|c| c.is_ascii_digit())) {
            return Err(RegisterError::InvalidInput("invalid phone".into()));
        }
    }

    let hash = hash_password(cfg, password).map_err(|e| RegisterError::Internal(e.to_string()))?;

    if let DbPool::Mysql(p) = pool {
        let mut tx = p.begin().await.map_err(|e| RegisterError::Internal(e.to_string()))?;

        let row = sqlx::query("SELECT COUNT(*) AS c FROM users WHERE status = 1")
            .fetch_one(&mut *tx).await.map_err(|e| RegisterError::Internal(e.to_string()))?;
        let c: i64 = row.try_get("c").unwrap_or(0);
        if c > 0 {
            return Err(RegisterError::Closed);
        }

        let existing: Option<(u64,)> = sqlx::query_as(
            "SELECT id FROM users WHERE username = ?",
        ).bind(username).fetch_optional(&mut *tx).await
            .map_err(|e| RegisterError::Internal(e.to_string()))?;
        if existing.is_some() {
            return Err(RegisterError::UsernameTaken);
        }

        let user_code = format!("U-{}", nanoid!(10));

        let res = sqlx::query(
            "INSERT INTO users (username, user_code, password_hash, email, phone, nickname, status) \
             VALUES (?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(username)
        .bind(&user_code)
        .bind(&hash)
        .bind(email.as_deref().unwrap_or(""))
        .bind(phone.as_deref().unwrap_or(""))
        .bind(nickname.as_deref().unwrap_or(username))
        .execute(&mut *tx)
        .await
        .map_err(|e| RegisterError::Internal(e.to_string()))?;
        let uid = res.last_insert_id();

        let role_row: Option<(u64,)> = sqlx::query_as(
            "SELECT id FROM roles WHERE code = 'admin'",
        ).fetch_optional(&mut *tx).await
            .map_err(|e| RegisterError::Internal(e.to_string()))?;
        let role_id = role_row.ok_or_else(|| RegisterError::Internal("admin role not seeded".into()))?.0;

        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
            .bind(uid).bind(role_id).execute(&mut *tx).await
            .map_err(|e| RegisterError::Internal(e.to_string()))?;

        tx.commit().await.map_err(|e| RegisterError::Internal(e.to_string()))?;
        Ok(uid)
    } else {
        Err(RegisterError::Internal("unsupported db".into()))
    }
}

fn is_valid_username(u: &str) -> bool {
    let len = u.chars().count();
    if !(3..=32).contains(&len) { return false; }
    u.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_hash_is_valid_phc() {
        assert!(PasswordHash::new(&dummy_hash()).is_ok());
    }

    #[test]
    fn truncate_ua_bounds() {
        let long = "a".repeat(300);
        assert_eq!(truncate_ua(&long).len(), 255);
        assert_eq!(truncate_ua("short"), "short");
    }

    #[test]
    fn valid_username_ok() {
        assert!(is_valid_username("admin"));
        assert!(is_valid_username("user.name_1-2"));
        assert!(!is_valid_username("ab"));
        assert!(!is_valid_username("a".repeat(33).as_str()));
        assert!(!is_valid_username("用户"));
        assert!(!is_valid_username("a b"));
    }
}
```

- [ ] **Step 3: 编译**

```powershell
cargo check -p user-service
```
Expected: 0 error。

- [ ] **Step 4: 跑模块测试**

```powershell
cargo test -p user-service --lib service::auth_svc
```
Expected: PASS（dummy hash parse / truncate_ua / valid_username）。

- [ ] **Step 5: Commit**

```bash
git add services/user-service/src/service/mod.rs \
        services/user-service/src/service/auth_svc.rs
git commit -m "feat(user-service): add auth_svc login/register/setup_status logic"
```

---

### Task 7: 路由 — POST /api/v1/auth/login + register + setup-status

**Files:**
- Modify: `services/user-service/src/routes/mod.rs`
- Create: `services/user-service/src/routes/auth.rs`

- [ ] **Step 1: 修改 routes/mod.rs**

`services/user-service/src/routes/mod.rs`：

```rust
pub mod auth;
pub mod health;
pub mod internal;
pub mod users;
pub mod roles;
pub mod permissions;
pub mod user_roles;
```

- [ ] **Step 2: 写 routes/auth.rs**

`services/user-service/src/routes/auth.rs`：

```rust
use crate::config::Config;
use crate::db::DbPool;
use crate::dto::{
    InvalidCredsResp, LockedResp, LoginReq, LoginSuccessResp, RegisterReq, RegisterResp,
    SetupStatusResp,
};
use crate::error::AppError;
use crate::service::auth_svc::{self, LoginCtx, LoginOutcome, RegisterError};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router, routing};
use std::net::SocketAddr;

#[derive(Clone)]
pub struct AuthState {
    pub pool: DbPool,
    pub redis: redis::Client,
    pub cfg: Config,
}

pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/api/v1/auth/login", routing::post(login))
        .route("/api/v1/auth/register", routing::post(register))
        .route("/api/v1/auth/setup-status", routing::get(setup_status))
        .with_state(state)
}

pub async fn login(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AuthState>,
    Json(req): Json<LoginReq>,
) -> Result<axum::response::Response, AppError> {
    if req.username.is_empty() || req.password.is_empty() {
        return Err(AppError::Validation("username and password are required".into()));
    }
    let ua = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
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
        ).into_response(),
        LoginOutcome::Locked { retry_after_secs } => (
            StatusCode::LOCKED,
            Json(LockedResp { error: "account locked", retry_after: retry_after_secs }),
        ).into_response(),
        LoginOutcome::Invalid => (
            StatusCode::UNAUTHORIZED,
            Json(InvalidCredsResp { error: "invalid credentials" }),
        ).into_response(),
    })
}

pub async fn register(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AuthState>,
    Json(req): Json<RegisterReq>,
) -> Result<axum::response::Response, AppError> {
    let _ip = client_ip(&headers, addr);
    let _ua = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("");
    let res = auth_svc::register_admin(
        &state.pool,
        &state.cfg.login_security,
        &req.username,
        &req.password,
        req.email.clone(),
        req.phone.clone(),
        req.nickname.clone(),
    ).await;
    Ok(match res {
        Ok(uid) => (
            StatusCode::CREATED,
            Json(RegisterResp { id: uid, username: req.username }),
        ).into_response(),
        Err(RegisterError::Closed) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "registration closed" })),
        ).into_response(),
        Err(RegisterError::UsernameTaken) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "username taken" })),
        ).into_response(),
        Err(RegisterError::InvalidInput(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": msg })),
        ).into_response(),
        Err(RegisterError::Internal(msg)) => {
            tracing::error!(error = %msg, "register_admin failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "internal" })),
            ).into_response()
        }
    })
}

pub async fn setup_status(
    State(state): State<AuthState>,
) -> Result<Json<SetupStatusResp>, AppError> {
    let required = auth_svc::setup_status(&state.pool).await?;
    Ok(Json(SetupStatusResp { setup_required: required }))
}

fn client_ip(headers: &HeaderMap, addr: SocketAddr) -> String {
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            return first.trim().to_string();
        }
    }
    addr.ip().to_string()
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

```powershell
cargo check -p user-service
```
Expected: 0 error。

- [ ] **Step 4: 跑路由单元测试**

```powershell
cargo test -p user-service --lib routes::auth
```
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add services/user-service/src/routes/mod.rs \
        services/user-service/src/routes/auth.rs
git commit -m "feat(user-service): add /api/v1/auth/{login,register,setup-status}"
```

---

### Task 8: main.rs 挂载路由

**Files:**
- Modify: `services/user-service/src/main.rs`

- [ ] **Step 1: 替换 main.rs**

`services/user-service/src/main.rs`：

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

```powershell
cargo check -p user-service
```
Expected: 0 error。需 workspace 已有 `tower-http`。

- [ ] **Step 3: Commit**

```bash
git add services/user-service/src/main.rs
git commit -m "feat(user-service): mount auth router in main"
```

---

### Task 9: user-service 集成测试

**Files:**
- Create: `services/user-service/tests/auth_e2e_test.rs`

- [ ] **Step 1: 写集成测试**

`services/user-service/tests/auth_e2e_test.rs`：

```rust
//! 集成测试：登录 + 注册 + setup-status
//! 跑：cargo test -p user-service --test auth_e2e_test -- --test-threads=1

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

struct Ctx {
    _mysql: ContainerAsync<Mysql>,
    _redis: ContainerAsync<Redis>,
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

    // 应用 migration；多语句默认不支持，分两条
    sqlx::query("CREATE DATABASE IF NOT EXISTS oa3").execute(&m_pool).await.ok();
    let oa3_url = format!("mysql://root:secret@{m_host}:{m_port}/oa3");
    let oa3_pool = MySqlPool::connect(&oa3_url).await.expect("oa3 connect");

    let migration = include_str!("../../migrations/2026_06_11_auth_login_and_register.sql");
    for stmt in migration.split(';') {
        let s = stmt.trim();
        if s.is_empty() { continue; }
        if let Err(e) = sqlx::query(s).execute(&oa3_pool).await {
            eprintln!("stmt failed: {} -- sql: {}", e, s);
        }
    }

    let redis = Redis::default().start().await.expect("start redis");
    let r_host = redis.get_host().await.expect("redis host");
    let r_port = redis.get_host_port_ipv4(6379).await;
    let redis_url = format!("redis://{r_host}:{r_port}/1");
    let redis_client = redis::Client::open(redis_url.clone()).expect("redis client");

    let cfg = Config {
        server: user_service::config::ServerCfg { host: "0.0.0.0".into(), port: 8082 },
        database: user_service::config::DatabaseCfg {
            db_type: user_service::config::DbType::Mysql,
            url: oa3_url,
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

    Ctx { _mysql: mysql, _redis: redis, pool: DbPool::Mysql(oa3_pool), redis_client, cfg }
}

fn hash(p: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(p.as_bytes(), &salt).unwrap().to_string()
}

fn login_ctx<'a>(ctx: &'a Ctx) -> LoginCtx<'a> {
    LoginCtx {
        pool: &ctx.pool, redis: &ctx.redis_client, cfg: &ctx.cfg.login_security,
        ip: "10.0.0.1".into(), user_agent: "test".into(),
    }
}

#[tokio::test]
async fn setup_required_true_when_empty() {
    let ctx = setup().await;
    let r = auth_svc::setup_status(&ctx.pool).await.unwrap();
    assert!(r);
}

#[tokio::test]
async fn register_first_user_becomes_admin() {
    let ctx = setup().await;
    let uid = auth_svc::register_admin(
        &ctx.pool, &ctx.cfg.login_security,
        "admin", "admin123", None, None, None,
    ).await.expect("register ok");
    assert!(uid > 0);

    if let DbPool::Mysql(p) = &ctx.pool {
        let row: (String,) = sqlx::query_as(
            "SELECT r.code FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = ?",
        ).bind(uid).fetch_one(p).await.unwrap();
        assert_eq!(row.0, "admin");
    }
    assert!(!auth_svc::setup_status(&ctx.pool).await.unwrap());
}

#[tokio::test]
async fn register_second_user_rejected() {
    let ctx = setup().await;
    auth_svc::register_admin(
        &ctx.pool, &ctx.cfg.login_security, "first", "pass1234", None, None, None,
    ).await.expect("first ok");
    let r = auth_svc::register_admin(
        &ctx.pool, &ctx.cfg.login_security, "second", "pass1234", None, None, None,
    ).await;
    assert!(matches!(r, Err(auth_svc::RegisterError::Closed)));
}

#[tokio::test]
async fn register_invalid_password() {
    let ctx = setup().await;
    let r = auth_svc::register_admin(
        &ctx.pool, &ctx.cfg.login_security, "admin", "123", None, None, None,
    ).await;
    assert!(matches!(r, Err(auth_svc::RegisterError::InvalidInput(_))));
}

#[tokio::test]
async fn login_success_clears_counter() {
    let ctx = setup().await;
    let username = "user1";
    if let DbPool::Mysql(p) = &ctx.pool {
        sqlx::query("INSERT INTO users (username, user_code, password_hash, status) VALUES (?, ?, ?, 1)")
            .bind(username).bind("U-1").bind(hash("right"))
            .execute(p).await.unwrap();
    }
    let lc = login_ctx(&ctx);
    auth_svc::login(&lc, username, "wrong").await.unwrap();
    let r = auth_svc::login(&lc, username, "right").await.unwrap();
    assert!(matches!(r, LoginOutcome::Success { .. }));

    let mut conn = ctx.redis_client.get_multiplexed_async_connection().await.unwrap();
    let n: i64 = conn.get(format!("auth:login:fail:{username}")).await.unwrap_or(0);
    assert_eq!(n, 0);
}

#[tokio::test]
async fn login_lock_after_max_failures() {
    let ctx = setup().await;
    let username = "user2";
    if let DbPool::Mysql(p) = &ctx.pool {
        sqlx::query("INSERT INTO users (username, user_code, password_hash, status) VALUES (?, ?, ?, 1)")
            .bind(username).bind("U-2").bind(hash("right"))
            .execute(p).await.unwrap();
    }
    let lc = login_ctx(&ctx);
    for _ in 0..ctx.cfg.login_security.max_failures {
        let r = auth_svc::login(&lc, username, "wrong").await.unwrap();
        assert!(matches!(r, LoginOutcome::Invalid));
    }
    let r = auth_svc::login(&lc, username, "right").await.unwrap();
    match r {
        LoginOutcome::Locked { retry_after_secs } => assert!(retry_after_secs > 0),
        _ => panic!("expected Locked"),
    }
}
```

- [ ] **Step 2: 跑集成测试**

```powershell
cd d:\projects\coder-micro-services
cargo test -p user-service --test auth_e2e_test -- --test-threads=1
```
Expected: 6 个测试全 PASS。需要 Docker 运行中。

- [ ] **Step 3: Commit**

```bash
git add services/user-service/tests/auth_e2e_test.rs
git commit -m "feat(user-service): integration tests for login + register + setup-status"
```

---

### Task 10: API 网关路由配置

**Files:**
- Delete: `docker/config/api-gateway/routes/credential-login.router.toml`
- Create: `docker/config/api-gateway/routes/auth.router.toml`

- [ ] **Step 1: 删 stub 路由文件**

```powershell
Remove-Item d:\projects\coder-micro-services\docker\config\api-gateway\routes\credential-login.router.toml
```

- [ ] **Step 2: 新建 auth 路由文件**

`docker/config/api-gateway/routes/auth.router.toml`：

```toml
[[route]]
match = "/api/v1/auth/**"
service = "user-service"
public = true
rewrite = { type = "strip_prefix", prefix = "/api/v1" }
```

- [ ] **Step 3: 校验**

```powershell
cd d:\projects\coder-micro-services
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes
```
Expected: 0 error。

- [ ] **Step 4: Commit**

```bash
git rm docker/config/api-gateway/routes/credential-login.router.toml
git add docker/config/api-gateway/routes/auth.router.toml
git commit -m "feat(api-gateway): route /api/v1/auth/** to user-service; drop stub"
```

---

### Task 11: token-service 启动时 client_secret 明文→哈希

**Files:**
- Modify: `services/token-service/src/store/client.rs`

- [ ] **Step 1: 读现有 client.rs 了解结构**

Run: `Get-Content d:\projects\coder-micro-services\services\token-service\src\store\client.rs`

记下 `Client` 结构与 `find_by_id` 等函数位置（用于下一步编辑）。

- [ ] **Step 2: 写失败测试**

在 `services/token-service/src/store/client.rs` 末尾追加（若文件无 `#[cfg(test)]` 模块则加）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_plaintext_secret_matches_argon2id() {
        let h = hash_client_secret_plaintext("dev-gateway-secret");
        assert!(argon2::password_hash::PasswordHash::new(&h).is_ok());
        // 验证可校验
        let ph = argon2::password_hash::PasswordHash::new(&h).unwrap();
        assert!(argon2::Argon2::default()
            .verify_password(b"dev-gateway-secret", &ph).is_ok());
    }

    #[test]
    fn maybe_upgrade_replaces_when_placeholder() {
        let mut c = Client {
            client_id: "api-gateway".into(),
            client_secret_hash: "$argon2id$v=19$m=65536,t=3,p=4$HERE".into(),
            name: "API Gateway".into(),
            permissions: vec![],
            grant_types: vec![],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
        };
        maybe_upgrade_client_secret(&mut c, "dev-gateway-secret");
        assert_ne!(c.client_secret_hash, "$argon2id$v=19$m=65536,t=3,p=4$HERE");
        let ph = argon2::password_hash::PasswordHash::new(&c.client_secret_hash).unwrap();
        assert!(argon2::Argon2::default()
            .verify_password(b"dev-gateway-secret", &ph).is_ok());
    }
}
```

同时在文件顶部 `pub struct Client { ... }` 后追加两个公开函数：

```rust
pub fn hash_client_secret_plaintext(plain: &str) -> String {
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::Argon2;
    use rand::rngs::OsRng;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(plain.as_bytes(), &salt)
        .expect("hash client secret").to_string()
}

pub fn maybe_upgrade_client_secret(c: &mut Client, default_plain: &str) {
    if c.client_secret_hash.starts_with("$argon2id$") && !c.client_secret_hash.contains("$HERE") {
        return;
    }
    c.client_secret_hash = hash_client_secret_plaintext(default_plain);
}
```

- [ ] **Step 3: 跑测试确认通过**

```powershell
cargo test -p token-service --lib store::client
```
Expected: PASS。

- [ ] **Step 4: 改 ClientStore 加载后升级**

在 `ClientStore` 实现里 `load` 或 `new` 后，对每个 `Client` 调用 `maybe_upgrade_client_secret(c, "dev-gateway-secret")`。具体调用点视现有结构而定（grep `client_secret_hash` 定位）。

典型插入点示例（视实际代码调整）：

```rust
impl ClientStore {
    pub async fn load(...) -> Self {
        // ... 现有加载逻辑
        for c in &mut clients {
            crate::store::client::maybe_upgrade_client_secret(c, "dev-gateway-secret");
        }
        Self { clients }
    }
}
```

- [ ] **Step 5: 编译**

```powershell
cargo check -p token-service
```
Expected: 0 error。

- [ ] **Step 6: Commit**

```bash
git add services/token-service/src/store/client.rs
git commit -m "feat(token-service): hash plaintext client_secret on startup"
```

---

### Task 12: 前端 Vite dev proxy

**Files:**
- Modify: `frontend/vite.config.ts`

- [ ] **Step 1: 改 server.proxy**

在 `frontend/vite.config.ts` 找到 `proxy: {},` 改为：

```ts
proxy: {
  "/api": {
    target: "http://localhost:3000",
    changeOrigin: true
  }
},
```

- [ ] **Step 2: 本地编译确认（无 type error）**

Run:
```powershell
cd d:\projects\coder-micro-services\frontend
pnpm typecheck
```
Expected: 无新增 error（已有 error 忽略）。

- [ ] **Step 3: Commit**

```bash
git add frontend/vite.config.ts
git commit -m "feat(frontend): vite dev proxy /api -> api-gateway :3000"
```

---

### Task 13: 前端 api/auth.ts 重写

**Files:**
- Modify: `frontend/src/api/auth.ts`

- [ ] **Step 1: 替换文件内容**

`frontend/src/api/auth.ts`：

```ts
import { http } from "@/utils/http";

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResult {
  code: string;
  expires_in: number;
}

export function login(data: LoginRequest) {
  return http.request<LoginResult>("post", "/api/v1/auth/login", { data });
}

export function logout() {
  return http.request<void>("post", "/api/v1/auth/logout");
}

export function getSetupStatus() {
  return http.request<{ setup_required: boolean }>(
    "get",
    "/api/v1/auth/setup-status"
  );
}

export interface RegisterRequest {
  username: string;
  password: string;
  email?: string;
  phone?: string;
  nickname?: string;
}

export interface RegisterResult {
  id: number;
  username: string;
}

export function register(data: RegisterRequest) {
  return http.request<RegisterResult>("post", "/api/v1/auth/register", { data });
}
```

- [ ] **Step 2: typecheck**

```powershell
cd d:\projects\coder-micro-services\frontend
pnpm typecheck
```
Expected: 无新增 error。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/api/auth.ts
git commit -m "feat(frontend): api/auth paths + register/setup-status"
```

---

### Task 14: 前端 api/token.ts 路径对齐

**Files:**
- Modify: `frontend/src/api/token.ts`

- [ ] **Step 1: 替换文件内容**

`frontend/src/api/token.ts`：

```ts
import { http } from "@/utils/http";

export interface AccessTokenRequest {
  grant_type: string;
  code: string;
  client_id: string;
  client_secret: string;
}

export interface AccessTokenResult {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
}

export function createAccessToken(data: AccessTokenRequest) {
  return http.request<AccessTokenResult>("post", "/api/v1/tokens/access", { data });
}

export interface RefreshTokenRequest {
  refresh_token: string;
  client_id: string;
  client_secret: string;
}

export function refreshToken(data: RefreshTokenRequest) {
  return http.request<AccessTokenResult>("post", "/api/v1/tokens/refresh", { data });
}

export function revokeToken(token: string) {
  return http.request<void>("post", "/api/v1/tokens/revoke", { data: { token } });
}

export function listResources() {
  return http.request<string[]>("get", "/api/v1/tokens/resource");
}

export function createApiKey(data: { name: string; scopes?: string[] }) {
  return http.request<{ api_key: string; id: string }>(
    "post",
    "/api/v1/tokens/api_key",
    { data }
  );
}
```

- [ ] **Step 2: typecheck**

```powershell
pnpm typecheck
```
Expected: 无新增 error。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/api/token.ts
git commit -m "feat(frontend): api/token paths to /api/v1/tokens/*"
```

---

### Task 15: 前端 api/user.ts 路径对齐

**Files:**
- Modify: `frontend/src/api/user.ts`

- [ ] **Step 1: 替换路径**

将 `frontend/src/api/user.ts` 中：
- `"/users/me"` → `"/api/v1/users/me"`
- `"/users/me"` (PUT) → `"/api/v1/users/me"`
- `"/users"` → `"/api/v1/users"`
- `` `/users/${id}` `` → `` `/api/v1/users/${id}` ``
- `` `/users/${id}/extends` `` → `` `/api/v1/users/${id}/extends` ``
- `` `/users/${id}/roles` `` → `` `/api/v1/users/${id}/roles` ``

最终文件：

```ts
import { http } from "@/utils/http";

export interface User {
  id: number;
  username: string;
  email: string;
  nickname: string;
  avatar: string;
  status: number;
  roles: string[];
  created_at: string;
  updated_at: string;
}

export interface ListUsersQuery {
  page?: number;
  size?: number;
  keyword?: string;
  status?: number;
}

export interface PageResult<T> {
  list: T[];
  total: number;
  page: number;
  size: number;
}

export function getCurrentUser() {
  return http.request<User>("get", "/api/v1/users/me");
}

export function updateCurrentUser(data: Partial<User>) {
  return http.request<User>("put", "/api/v1/users/me", { data });
}

export function listUsers(params: ListUsersQuery) {
  return http.request<PageResult<User>>("get", "/api/v1/users", { params });
}

export function getUser(id: number) {
  return http.request<User>("get", `/api/v1/users/${id}`);
}

export function patchUserExtends(id: number, data: Record<string, unknown>) {
  return http.request<User>("patch", `/api/v1/users/${id}/extends`, { data });
}

export function assignUserRoles(id: number, roleIds: number[]) {
  return http.request<void>("post", `/api/v1/users/${id}/roles`, {
    data: { role_ids: roleIds }
  });
}

export function removeUserRoles(id: number, roleIds: number[]) {
  return http.request<void>("delete", `/api/v1/users/${id}/roles`, {
    data: { role_ids: roleIds }
  });
}
```

- [ ] **Step 2: typecheck**

```powershell
pnpm typecheck
```
Expected: 无新增 error。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/api/user.ts
git commit -m "feat(frontend): api/user paths to /api/v1/users/*"
```

---

### Task 16: 前端 http 白名单

**Files:**
- Modify: `frontend/src/utils/http/index.ts`

- [ ] **Step 1: 改白名单**

`frontend/src/utils/http/index.ts` 找到：

```ts
const whiteList = ["/refresh-token", "/login"];
```

改为：

```ts
const whiteList = ["/api/v1/auth/", "/api/v1/tokens/access", "/api/v1/tokens/refresh"];
```

- [ ] **Step 2: typecheck**

```powershell
pnpm typecheck
```
Expected: 无新增 error。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/utils/http/index.ts
git commit -m "feat(frontend): http whitelist for /api/v1/auth + /api/v1/tokens"
```

---

### Task 17: 前端 login 视图加 setup 检测

**Files:**
- Modify: `frontend/src/views/login/index.vue`

- [ ] **Step 1: 修改 script**

在 `frontend/src/views/login/index.vue` `<script setup>` 内：
- import 增加 `import { getSetupStatus } from "@/api/auth";` 与 `import { onMounted, ref } from "vue";`（已有 ref）
- 加 `const setupRequired = ref(false);`
- 加 `onMounted(async () => { const r = await getSetupStatus(); setupRequired.value = r.data.setup_required; if (setupRequired.value) router.push("/register"); });`

- [ ] **Step 2: 修改 template**

在 `<el-form-item>`（含登录按钮）之后，加：

```html
<div v-if="setupRequired" class="mt-2 text-center">
  <router-link to="/register" class="text-primary">
    首次部署？注册管理员
  </router-link>
</div>
```

- [ ] **Step 3: typecheck + 启动 dev 视觉确认（可选）**

```powershell
pnpm typecheck
```
Expected: 无新增 error。

- [ ] **Step 4: Commit**

```bash
git add frontend/src/views/login/index.vue
git commit -m "feat(frontend): login view detects setup and links to register"
```

---

### Task 18: 前端注册页 register.vue

**Files:**
- Create: `frontend/src/views/login/register.vue`

- [ ] **Step 1: 新建文件**

`frontend/src/views/login/register.vue`：

```vue
<script setup lang="ts">
import { ref, reactive } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { message } from "@/utils/message";
import { useUserStoreHook } from "@/store/modules/user";
import { register, getSetupStatus } from "@/api/auth";
import Lock from "~icons/ri/lock-fill";
import User from "~icons/ri/user-3-fill";
import { avatar } from "./utils/static";

defineOptions({ name: "Register" });

const router = useRouter();
const { t } = useI18n();
const loading = ref(false);

const form = reactive({
  username: "",
  password: "",
  confirm: "",
  email: ""
});

const onRegister = async () => {
  if (form.password !== form.confirm) {
    message("两次密码不一致", { type: "error" });
    return;
  }
  loading.value = true;
  try {
    const status = await getSetupStatus();
    if (!status.data.setup_required) {
      message("注册已关闭", { type: "error" });
      router.push("/login");
      return;
    }
    await register({
      username: form.username,
      password: form.password,
      email: form.email || undefined
    });
    await useUserStoreHook().loginByUsername({
      username: form.username,
      password: form.password
    });
    await useUserStoreHook().setUserInfo();
    message("注册成功", { type: "success" });
    router.push("/welcome");
  } catch (e: any) {
    const msg = e?.response?.data?.error ?? "注册失败";
    message(msg, { type: "error" });
  } finally {
    loading.value = false;
  }
};
</script>

<template>
  <div class="select-none">
    <div class="login-container">
      <div class="login-form">
        <avatar class="avatar" />
        <h2 class="outline-hidden">注册管理员</h2>
        <el-form :model="form" size="large" @submit.prevent>
          <el-form-item>
            <el-input v-model="form.username" clearable placeholder="用户名">
              <template #prefix><User /></template>
            </el-input>
          </el-form-item>
          <el-form-item>
            <el-input v-model="form.password" show-password clearable placeholder="密码（≥6 位）">
              <template #prefix><Lock /></template>
            </el-input>
          </el-form-item>
          <el-form-item>
            <el-input v-model="form.confirm" show-password clearable placeholder="确认密码">
              <template #prefix><Lock /></template>
            </el-input>
          </el-form-item>
          <el-form-item>
            <el-input v-model="form.email" clearable placeholder="邮箱（可选）" />
          </el-form-item>
          <el-form-item>
            <el-button class="w-full" type="primary" :loading="loading" @click="onRegister">
              注册并登录
            </el-button>
          </el-form-item>
          <el-form-item>
            <router-link to="/login" class="text-primary">返回登录</router-link>
          </el-form-item>
        </el-form>
      </div>
    </div>
  </div>
</template>

<style scoped>
@import url("@/style/login.css");
</style>
```

- [ ] **Step 2: typecheck**

```powershell
pnpm typecheck
```
Expected: 无新增 error。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/views/login/register.vue
git commit -m "feat(frontend): add register page"
```

---

### Task 19: 前端路由加 /register

**Files:**
- Modify: `frontend/src/router/modules/account.ts`
- Modify: `frontend/src/router/index.ts`

- [ ] **Step 1: 修改 account.ts**

`frontend/src/router/modules/account.ts` 末尾 `] satisfies RouteConfigsTable;` 之前追加：

```ts
{
  path: "/register",
  name: "Register",
  component: () => import("@/views/login/register.vue"),
  meta: { title: "注册管理员", showLink: false, hidden: true }
}
```

- [ ] **Step 2: 修改 router 白名单**

`frontend/src/router/index.ts` 找到：

```ts
const whiteList = ["/login"];
```

改为：

```ts
const whiteList = ["/login", "/register"];
```

- [ ] **Step 3: typecheck**

```powershell
pnpm typecheck
```
Expected: 无新增 error。

- [ ] **Step 4: Commit**

```bash
git add frontend/src/router/modules/account.ts \
        frontend/src/router/index.ts
git commit -m "feat(frontend): add /register route to whitelist"
```

---

### Task 20: 前端 .env 加 client secret

**Files:**
- Modify: `frontend/.env.development`

- [ ] **Step 1: 追加变量**

`frontend/.env.development` 末尾追加：

```
VITE_CLIENT_SECRET = dev-gateway-secret
```

- [ ] **Step 2: Commit**

```bash
git add frontend/.env.development
git commit -m "feat(frontend): VITE_CLIENT_SECRET for dev token exchange"
```

---

### Task 21: 全量验证

**Files:** (无)

- [ ] **Step 1: fmt + clippy**

```powershell
cd d:\projects\coder-micro-services
cargo fmt --all
cargo clippy -p user-service --all-targets -- -D warnings
cargo clippy -p token-service --all-targets -- -D warnings
```
Expected: 0 warning。

- [ ] **Step 2: user-service 全量 lib 测试**

```powershell
cargo test -p user-service --lib
```
Expected: PASS。

- [ ] **Step 3: user-service 集成测试**

```powershell
cargo test -p user-service --test auth_e2e_test -- --test-threads=1
```
Expected: 6 个测试全 PASS。

- [ ] **Step 4: token-service lib 测试**

```powershell
cargo test -p token-service --lib
```
Expected: PASS。

- [ ] **Step 5: 网关路由再次校验**

```powershell
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes
```
Expected: 0 error。

- [ ] **Step 6: 前端 typecheck**

```powershell
cd d:\projects\coder-micro-services\frontend
pnpm typecheck
```
Expected: 无新增 error。

- [ ] **Step 7: 提交任何自动格式化产物**

```bash
git status
# 若有改动：
git add -u
git commit -m "chore: apply fmt and clippy fixes"
```

---

### Task 22: 端到端冒烟

**Files:** (无)

- [ ] **Step 1: 启动基础设施**

```powershell
cd d:\projects\coder-micro-services
docker compose -f infrastructure\docker-compose.yml up -d
```

- [ ] **Step 2: 应用迁移**

```powershell
# 假设 mysql 容器名为 coder-mysql，oa3 库
Get-Content migrations\2026_06_11_auth_login_and_register.sql |
  docker exec -i coder-mysql mysql -uroot -psecret
```
（实际容器名/密码按 `docker-compose.yml` 调整。）

- [ ] **Step 3: 启动三个 Rust 服务**

在三个真实 PowerShell（vcvarsall x64）窗口分别：

```powershell
cd d:\projects\coder-micro-services
cargo run -p user-service
```

```powershell
cd d:\projects\coder-micro-services
cargo run -p token-service
```

```powershell
cd d:\projects\coder-micro-services
cargo run -p api-gateway
```

- [ ] **Step 4: 启动前端**

```powershell
cd d:\projects\coder-micro-services\frontend
pnpm dev
```

- [ ] **Step 5: 浏览器验证**

打开 `http://localhost:8848/#/login`：
- 应自动跳 `/register`
- 填 `admin` / `admin123` 提交 → 跳 `/welcome`
- 退出 → 跳 `/login` → `admin` / `admin123` → 跳 `/welcome`
- 故意输错 6 次 → 应提示"账号被锁" + retry_after

- [ ] **Step 6: 提交验证结果**

```bash
git status
# 如有未提交改动：
git add -u
git commit -m "chore: e2e smoke verified admin/admin123 login"
```

---

## 自检（与 spec 对齐）

- 后端 `/api/v1/auth/login` → Task 7
- 后端 `/api/v1/auth/register`（首位=admin）→ Task 6 + Task 7
- 后端 `/api/v1/auth/setup-status` → Task 6 + Task 7
- 网关 `auth.router.toml` + 删 stub → Task 10
- token-service 启动哈希 client_secret → Task 11
- 迁移 SQL + 角色预置 → Task 2
- user-service 集成测试（登录+注册+setup）→ Task 9
- 前端 vite proxy → Task 12
- 前端 api 路径统一 v1 → Task 13/14/15
- 前端 http 白名单 → Task 16
- 前端 login 视图 setup 检测 → Task 17
- 前端 register 页 → Task 18
- 前端路由 /register → Task 19
- 前端 .env client_secret → Task 20
- 端到端冒烟 → Task 22

无占位符；类型/函数名跨 task 一致（`register_admin` / `setup_status` / `login` / `AuthState` / `LoginCtx` / `RegisterError`）。
