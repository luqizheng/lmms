# iam-service 合并实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 合并 token-service、credential-login-service 和 user-service 为统一的 iam-service。

**Architecture:** 
- 创建新服务 `services/iam-service/`，整合认证、用户、角色、权限、菜单、部门、Token 管理能力
- 复用 libs 层：common-error、common-log、common-config、common-db、common-cache、jwt-ctx、authz
- 端点域统一为 `/iam/*`

**Tech Stack:** Rust、Axum、SQLx、Redis、JWT、Argon2

---

## 文件结构

```
services/iam-service/
├── src/
│   ├── main.rs              # 服务入口
│   ├── lib.rs               # 模块导出
│   ├── config.rs            # 配置加载
│   ├── db.rs                # 数据库连接
│   ├── error.rs             # 错误定义
│   ├── auth.rs              # JWT 认证中间件
│   ├── state.rs             # AppState
│   ├── models/              # 数据库模型
│   │   ├── user.rs          # 用户
│   │   ├── role.rs          # 角色
│   │   ├── permission.rs    # 权限
│   │   ├── menu.rs          # 菜单
│   │   ├── org_unit.rs      # 部门
│   │   ├── login_log.rs     # 登录日志
│   │   ├── refresh_token.rs # 刷新令牌
│   │   └── mod.rs
│   ├── dto/                 # 请求/响应 DTO
│   │   ├── auth.rs          # 认证相关
│   │   ├── user.rs          # 用户相关
│   │   ├── role.rs          # 角色相关
│   │   ├── permission.rs    # 权限相关
│   │   ├── menu.rs          # 菜单相关
│   │   ├── org_unit.rs      # 部门相关
│   │   └── mod.rs
│   ├── service/             # 业务服务层
│   │   ├── auth_svc.rs      # 认证服务
│   │   ├── user_svc.rs      # 用户服务
│   │   ├── role_svc.rs      # 角色服务
│   │   ├── permission_svc.rs # 权限服务
│   │   ├── menu_svc.rs      # 菜单服务
│   │   ├── org_svc.rs       # 部门服务
│   │   ├── token_svc.rs     # Token 服务
│   │   └── mod.rs
│   └── routes/              # API 路由
│       ├── auth.rs          # /iam/auth/*
│       ├── users.rs         # /iam/users/*
│       ├── roles.rs         # /iam/roles/*
│       ├── permissions.rs   # /iam/permissions/*
│       ├── menus.rs         # /iam/menus/*
│       ├── org_units.rs     # /iam/org-units/*
│       ├── tokens.rs        # /iam/tokens/*
│       ├── sessions.rs      # /iam/sessions/*
│       └── mod.rs
├── migrations/              # 数据库迁移
│   ├── mysql/               # MySQL 迁移
│   └── sqlite/              # SQLite 迁移（测试）
├── config/                  # 配置文件
│   └── config.yaml
├── tests/                   # 集成测试
│   ├── auth_test.rs
│   ├── user_test.rs
│   ├── role_test.rs
│   └── integration_test.rs
├── Cargo.toml
└── Dockerfile
```

---

## Task 1: 创建 iam-service 基础结构

**Files:**
- Create: `services/iam-service/Cargo.toml`
- Create: `services/iam-service/src/lib.rs`
- Create: `services/iam-service/src/main.rs`
- Create: `services/iam-service/config/config.yaml`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "iam-service"
version = "0.1.0"
edition = "2021"
default-run = "iam-service"

[dependencies]
common-error = { path = "../../libs/common-error" }
common-log = { path = "../../libs/common-log" }
common-config = { path = "../../libs/common-config" }
common-db = { path = "../../libs/common-db", features = ["mysql", "sqlite"] }
common-cache = { path = "../../libs/common-cache", features = ["redis"] }
common-crypto = { path = "../../libs/common-crypto" }
common-validator = { path = "../../libs/common-validator" }
jwt-ctx = { path = "../../libs/jwt-ctx" }
authz = { path = "../../libs/authz" }
consul-registrar = { path = "../../libs/consul-registrar" }

axum.workspace = true
tokio.workspace = true
tower.workspace = true
tower-http.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
serde.workspace = true
serde_json.workspace = true
jwt-simple.workspace = true
uuid.workspace = true
argon2.workspace = true
thiserror.workspace = true
rand.workspace = true
base64.workspace = true
rsa.workspace = true
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls-aws-lc-rs", "mysql", "sqlite", "macros", "chrono", "json"] }
serde_yaml = "0.9"
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
redis.workspace = true
nanoid = "0.4"

[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["mysql", "redis"] }
reqwest.workspace = true
mockall = "0.13"
```

- [ ] **Step 2: 创建 lib.rs**

```rust
pub mod auth;
pub mod config;
pub mod db;
pub mod dto;
pub mod error;
pub mod models;
pub mod routes;
pub mod service;
pub mod state;

pub use config::{load_config, Config};
pub use consul_registrar::ConsulRegistrar;
```

- [ ] **Step 3: 创建 main.rs**

```rust
use iam_service::{config::load_config, db, routes, state::AppState};
use common_log::setup_logging;
use consul_registrar::ConsulRegistrar;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging();
    
    let config = load_config()?;
    info!("Loaded config: {:?}", config.app.name);
    
    let db_pool = db::init_pool(&config.database).await?;
    let redis_client = common_cache::new_redis_client(&config.redis).await?;
    
    let state = AppState::new(db_pool, redis_client, config.clone());
    
    let app = routes::build_router(state);
    
    let addr = format!("{}:{}", config.app.host, config.app.port).parse()?;
    info!("Starting iam-service on {}", addr);
    
    #[cfg(not(test))]
    {
        let _registrar = ConsulRegistrar::new(
            &config.consul,
            "iam-service",
            addr.to_string(),
        ).await?;
    }
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}
```

- [ ] **Step 4: 创建 config/config.yaml**

```yaml
app:
  name: iam-service
  host: 0.0.0.0
  port: 3002

database:
  url: mysql://root:password@localhost:3306/iam_db
  max_connections: 20
  min_connections: 5

redis:
  url: redis://localhost:6379/0
  ttl_sec: 86400

jwt:
  private_key_path: keys/private.pem
  public_key_path: keys/public.pem
  access_token_ttl_minutes: 15
  refresh_token_ttl_days: 7

consul:
  host: localhost
  port: 8500
  service_name: iam-service
```

- [ ] **Step 5: 初始化项目**

Run: `cd services/iam-service && cargo init`
Expected: 创建基础项目结构

- [ ] **Step 6: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 2: 创建配置和状态模块

**Files:**
- Create: `services/iam-service/src/config.rs`
- Create: `services/iam-service/src/state.rs`
- Create: `services/iam-service/src/db.rs`

- [ ] **Step 1: 创建 config.rs**

```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub jwt: JwtConfig,
    pub consul: ConsulConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub ttl_sec: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub private_key_path: PathBuf,
    pub public_key_path: PathBuf,
    pub access_token_ttl_minutes: u64,
    pub refresh_token_ttl_days: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConsulConfig {
    pub host: String,
    pub port: u16,
    pub service_name: String,
}

pub fn load_config() -> anyhow::Result<Config> {
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config/config.yaml".to_string());
    
    let content = std::fs::read_to_string(config_path)?;
    let config: Config = serde_yaml::from_str(&content)?;
    
    Ok(config)
}
```

- [ ] **Step 2: 创建 state.rs**

```rust
use common_db::DbPool;
use common_cache::RedisClient;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub redis: RedisClient,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: DbPool, redis: RedisClient, config: Config) -> Self {
        Self {
            db,
            redis,
            config: Arc::new(config),
        }
    }
}

pub type State = axum::extract::State<AppState>;
```

- [ ] **Step 3: 创建 db.rs**

```rust
use common_db::{init_pool, DbPool};
use crate::config::DatabaseConfig;

pub async fn init_pool(config: &DatabaseConfig) -> anyhow::Result<DbPool> {
    let pool = init_pool(&config.url, config.max_connections, config.min_connections).await?;
    Ok(pool)
}
```

- [ ] **Step 4: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 3: 创建错误和认证模块

**Files:**
- Create: `services/iam-service/src/error.rs`
- Create: `services/iam-service/src/auth.rs`

- [ ] **Step 1: 创建 error.rs**

```rust
use common_error::{AppError, ErrorCode};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IamError {
    #[error("invalid credentials")]
    InvalidCredentials,
    
    #[error("user not found")]
    UserNotFound,
    
    #[error("user disabled")]
    UserDisabled,
    
    #[error("password expired")]
    PasswordExpired,
    
    #[error("token expired")]
    TokenExpired,
    
    #[error("token revoked")]
    TokenRevoked,
    
    #[error("invalid token")]
    InvalidToken,
    
    #[error("permission denied")]
    PermissionDenied,
    
    #[error("role not found")]
    RoleNotFound,
    
    #[error("permission not found")]
    PermissionNotFound,
    
    #[error("menu not found")]
    MenuNotFound,
    
    #[error("org unit not found")]
    OrgUnitNotFound,
    
    #[error("duplicate user")]
    DuplicateUser,
    
    #[error("duplicate role")]
    DuplicateRole,
    
    #[error("duplicate permission")]
    DuplicatePermission,
    
    #[error("duplicate menu")]
    DuplicateMenu,
    
    #[error("duplicate org unit")]
    DuplicateOrgUnit,
    
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("jwt error: {0}")]
    Jwt(#[from] jwt_simple::Error),
    
    #[error("crypto error: {0}")]
    Crypto(#[from] common_crypto::CryptoError),
    
    #[error("validation error: {0}")]
    Validation(#[from] validator::ValidationErrors),
}

impl From<IamError> for AppError {
    fn from(e: IamError) -> Self {
        match e {
            IamError::InvalidCredentials => AppError::unauthorized(ErrorCode::IAM_AUTH_1001, e.to_string()),
            IamError::UserNotFound => AppError::not_found(ErrorCode::IAM_USER_2001, e.to_string()),
            IamError::UserDisabled => AppError::forbidden(ErrorCode::IAM_USER_2002, e.to_string()),
            IamError::PasswordExpired => AppError::unauthorized(ErrorCode::IAM_AUTH_1002, e.to_string()),
            IamError::TokenExpired => AppError::unauthorized(ErrorCode::IAM_TOKEN_3001, e.to_string()),
            IamError::TokenRevoked => AppError::unauthorized(ErrorCode::IAM_TOKEN_3002, e.to_string()),
            IamError::InvalidToken => AppError::unauthorized(ErrorCode::IAM_TOKEN_3003, e.to_string()),
            IamError::PermissionDenied => AppError::forbidden(ErrorCode::IAM_AUTHZ_4001, e.to_string()),
            IamError::RoleNotFound => AppError::not_found(ErrorCode::IAM_ROLE_5001, e.to_string()),
            IamError::PermissionNotFound => AppError::not_found(ErrorCode::IAM_PERM_6001, e.to_string()),
            IamError::MenuNotFound => AppError::not_found(ErrorCode::IAM_MENU_7001, e.to_string()),
            IamError::OrgUnitNotFound => AppError::not_found(ErrorCode::IAM_ORG_8001, e.to_string()),
            IamError::DuplicateUser => AppError::conflict(ErrorCode::IAM_USER_2003, e.to_string()),
            IamError::DuplicateRole => AppError::conflict(ErrorCode::IAM_ROLE_5002, e.to_string()),
            IamError::DuplicatePermission => AppError::conflict(ErrorCode::IAM_PERM_6002, e.to_string()),
            IamError::DuplicateMenu => AppError::conflict(ErrorCode::IAM_MENU_7002, e.to_string()),
            IamError::DuplicateOrgUnit => AppError::conflict(ErrorCode::IAM_ORG_8002, e.to_string()),
            IamError::Database(e) => AppError::internal(ErrorCode::IAM_SYS_9001, e.to_string()),
            IamError::Redis(e) => AppError::internal(ErrorCode::IAM_SYS_9002, e.to_string()),
            IamError::Jwt(e) => AppError::internal(ErrorCode::IAM_SYS_9003, e.to_string()),
            IamError::Crypto(e) => AppError::internal(ErrorCode::IAM_SYS_9004, e.to_string()),
            IamError::Validation(e) => AppError::bad_request(ErrorCode::IAM_SYS_9005, e.to_string()),
        }
    }
}
```

- [ ] **Step 2: 创建 auth.rs**

```rust
use axum::{
    extract::{FromRequestParts, TypedHeader},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use jwt_ctx::{Principal, TokenExtractor};
use tracing::info;

pub async fn auth_middleware<B>(
    TokenExtractor(principal): TokenExtractor,
    mut req: axum::http::Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    req.extensions_mut().insert(principal.clone());
    Ok(next.run(req).await)
}

pub async fn require_permission<B>(
    permission: &'static str,
    principal: Principal,
    req: axum::http::Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if !principal.has_permission(permission) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
}

pub async fn require_role<B>(
    role: &'static str,
    principal: Principal,
    req: axum::http::Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if !principal.has_role(role) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
}
```

- [ ] **Step 3: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 4: 创建数据库模型

**Files:**
- Create: `services/iam-service/src/models/mod.rs`
- Create: `services/iam-service/src/models/user.rs`
- Create: `services/iam-service/src/models/role.rs`
- Create: `services/iam-service/src/models/permission.rs`
- Create: `services/iam-service/src/models/menu.rs`
- Create: `services/iam-service/src/models/org_unit.rs`
- Create: `services/iam-service/src/models/login_log.rs`
- Create: `services/iam-service/src/models/refresh_token.rs`

- [ ] **Step 1: 创建 models/mod.rs**

```rust
pub mod login_log;
pub mod menu;
pub mod org_unit;
pub mod permission;
pub mod refresh_token;
pub mod role;
pub mod user;

pub use login_log::LoginLog;
pub use menu::Menu;
pub use org_unit::OrgUnit;
pub use permission::Permission;
pub use refresh_token::RefreshToken;
pub use role::Role;
pub use user::User;
```

- [ ] **Step 2: 创建 models/user.rs**

```rust
use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub password_hash: String,
    pub salt: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub status: UserStatus,
    pub email_verified: bool,
    pub phone_verified: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub last_login_ip: Option<String>,
    pub password_expired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum UserStatus {
    #[sqlx(rename = "active")]
    Active,
    #[sqlx(rename = "disabled")]
    Disabled,
    #[sqlx(rename = "locked")]
    Locked,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct UserRole {
    pub user_id: String,
    pub role_id: String,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 3: 创建 models/role.rs**

```rust
use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    pub data_scope: DataScope,
    pub status: RoleStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum RoleStatus {
    #[sqlx(rename = "active")]
    Active,
    #[sqlx(rename = "disabled")]
    Disabled,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum DataScope {
    #[sqlx(rename = "all")]
    All,
    #[sqlx(rename = "org")]
    Org,
    #[sqlx(rename = "dept")]
    Dept,
    #[sqlx(rename = "self")]
    Self,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct RolePermission {
    pub role_id: String,
    pub permission_id: String,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 4: 创建 models/permission.rs**

```rust
use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Permission {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub category: PermissionCategory,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub sort_order: i32,
    pub status: PermissionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum PermissionCategory {
    #[sqlx(rename = "menu")]
    Menu,
    #[sqlx(rename = "button")]
    Button,
    #[sqlx(rename = "api")]
    Api,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum PermissionStatus {
    #[sqlx(rename = "enabled")]
    Enabled,
    #[sqlx(rename = "disabled")]
    Disabled,
}
```

- [ ] **Step 5: 创建 models/menu.rs**

```rust
use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Menu {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub path: String,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub visible: bool,
    pub status: MenuStatus,
    pub permission_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum MenuStatus {
    #[sqlx(rename = "enabled")]
    Enabled,
    #[sqlx(rename = "disabled")]
    Disabled,
}
```

- [ ] **Step 6: 创建 models/org_unit.rs**

```rust
use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct OrgUnit {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub level: i32,
    pub sort_order: i32,
    pub status: OrgStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct UserOrgUnit {
    pub user_id: String,
    pub org_unit_id: String,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum OrgStatus {
    #[sqlx(rename = "active")]
    Active,
    #[sqlx(rename = "disabled")]
    Disabled,
}
```

- [ ] **Step 7: 创建 models/login_log.rs**

```rust
use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct LoginLog {
    pub id: String,
    pub user_id: Option<String>,
    pub username: String,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub location: Option<String>,
    pub login_type: LoginType,
    pub success: bool,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum LoginType {
    #[sqlx(rename = "password")]
    Password,
    #[sqlx(rename = "sms")]
    Sms,
    #[sqlx(rename = "sso")]
    Sso,
}
```

- [ ] **Step 8: 创建 models/refresh_token.rs**

```rust
use sqlx::{FromRow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct RefreshToken {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub client_ip: String,
    pub user_agent: Option<String>,
    pub revoked: bool,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 9: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 5: 创建 DTO 模块

**Files:**
- Create: `services/iam-service/src/dto/mod.rs`
- Create: `services/iam-service/src/dto/auth.rs`
- Create: `services/iam-service/src/dto/user.rs`
- Create: `services/iam-service/src/dto/role.rs`

- [ ] **Step 1: 创建 dto/mod.rs**

```rust
pub mod auth;
pub mod menu;
pub mod org_unit;
pub mod permission;
pub mod role;
pub mod user;

pub use auth::*;
pub use menu::*;
pub use org_unit::*;
pub use permission::*;
pub use role::*;
pub use user::*;
```

- [ ] **Step 2: 创建 dto/auth.rs**

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 1, max = 100))]
    pub username: String,
    #[validate(length(min = 6, max = 128))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RefreshRequest {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 6, max = 128))]
    pub old_password: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub login_at: String,
}
```

- [ ] **Step 3: 创建 dto/user.rs**

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::DateTime;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 1, max = 50))]
    pub username: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 11, max = 11))]
    pub phone: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub nickname: String,
    pub avatar: Option<String>,
    pub role_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 100))]
    pub nickname: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 11, max = 11))]
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserQuery {
    pub keyword: Option<String>,
    pub status: Option<String>,
    pub org_unit_id: Option<String>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: String,
    pub avatar: Option<String>,
    pub status: String,
    pub email_verified: bool,
    pub phone_verified: bool,
    pub last_login_at: Option<DateTime<chrono::Utc>>,
    pub last_login_ip: Option<String>,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserListResponse {
    pub items: Vec<UserResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize)]
pub struct BatchImportUsersRequest {
    pub users: Vec<CreateUserRequest>,
}
```

- [ ] **Step 4: 创建 dto/role.rs**

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::DateTime;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRoleRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 50))]
    pub code: String,
    pub description: Option<String>,
    #[validate(range(min = 0, max = 3))]
    pub data_scope: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRoleRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    pub description: Option<String>,
    #[validate(range(min = 0, max = 3))]
    pub data_scope: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RoleQuery {
    pub keyword: Option<String>,
    pub status: Option<String>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: String,
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    pub data_scope: String,
    pub status: String,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct RoleListResponse {
    pub items: Vec<RoleResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize)]
pub struct AssignPermissionsRequest {
    pub permission_ids: Vec<String>,
}
```

- [ ] **Step 5: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 6: 创建剩余 DTO 模块

**Files:**
- Create: `services/iam-service/src/dto/permission.rs`
- Create: `services/iam-service/src/dto/menu.rs`
- Create: `services/iam-service/src/dto/org_unit.rs`

- [ ] **Step 1: 创建 dto/permission.rs**

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::DateTime;

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePermissionRequest {
    #[validate(length(min = 1, max = 100))]
    pub code: String,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub description: Option<String>,
    #[validate(range(min = 0, max = 2))]
    pub category: i32,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePermissionRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PermissionQuery {
    pub keyword: Option<String>,
    pub category: Option<i32>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize)]
pub struct PermissionResponse {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub sort_order: i32,
    pub status: String,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct PermissionListResponse {
    pub items: Vec<PermissionResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}
```

- [ ] **Step 2: 创建 dto/menu.rs**

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::DateTime;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateMenuRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 100))]
    pub code: String,
    #[validate(length(min = 1, max = 255))]
    pub path: String,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: Option<i32>,
    pub visible: Option<bool>,
    pub permission_id: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateMenuRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub code: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub path: Option<String>,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: Option<i32>,
    pub visible: Option<bool>,
    pub permission_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MenuResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub path: String,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub visible: bool,
    pub status: String,
    pub permission_id: Option<String>,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
    pub children: Vec<MenuResponse>,
}

#[derive(Debug, Serialize)]
pub struct MenuListResponse {
    pub items: Vec<MenuResponse>,
    pub total: u64,
}
```

- [ ] **Step 3: 创建 dto/org_unit.rs**

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::DateTime;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrgUnitRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 50))]
    pub code: String,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateOrgUnitRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub code: Option<String>,
    pub sort_order: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrgUnitResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub level: i32,
    pub sort_order: i32,
    pub status: String,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
    pub children: Vec<OrgUnitResponse>,
}

#[derive(Debug, Deserialize)]
pub struct AssignUsersRequest {
    pub user_ids: Vec<String>,
}
```

- [ ] **Step 4: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 7: 创建服务层模块

**Files:**
- Create: `services/iam-service/src/service/mod.rs`
- Create: `services/iam-service/src/service/auth_svc.rs`
- Create: `services/iam-service/src/service/user_svc.rs`

- [ ] **Step 1: 创建 service/mod.rs**

```rust
pub mod auth_svc;
pub mod menu_svc;
pub mod org_svc;
pub mod permission_svc;
pub mod role_svc;
pub mod token_svc;
pub mod user_svc;

pub use auth_svc::AuthService;
pub use menu_svc::MenuService;
pub use org_svc::OrgService;
pub use permission_svc::PermissionService;
pub use role_svc::RoleService;
pub use token_svc::TokenService;
pub use user_svc::UserService;
```

- [ ] **Step 2: 创建 service/auth_svc.rs**

```rust
use crate::{
    config::Config,
    error::IamError,
    models::{LoginLog, RefreshToken, User},
    state::AppState,
    dto::{LoginRequest, LoginResponse, RefreshResponse, ChangePasswordRequest, UserInfo},
};
use common_crypto::password::{hash_password, verify_password};
use jwt_ctx::Principal;
use chrono::{DateTime, Utc};
use nanoid::nanoid;
use tracing::info;

pub struct AuthService {
    state: AppState,
}

impl AuthService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse, IamError> {
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE username = ? AND deleted_at IS NULL",
            req.username
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::InvalidCredentials)?;

        if user.status != UserStatus::Active {
            return Err(IamError::UserDisabled);
        }

        if let Some(expired_at) = user.password_expired_at {
            if expired_at < Utc::now() {
                return Err(IamError::PasswordExpired);
            }
        }

        if !verify_password(&req.password, &user.password_hash, &user.salt)? {
            self.log_login_failure(&req.username, "password mismatch").await?;
            return Err(IamError::InvalidCredentials);
        }

        let (access_token, refresh_token) = self.generate_tokens(&user).await?;

        self.update_last_login(&user.id).await?;
        self.log_login_success(&user).await?;

        let user_info = self.build_user_info(&user).await?;

        Ok(LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.state.config.jwt.access_token_ttl_minutes * 60,
            user: user_info,
        })
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<RefreshResponse, IamError> {
        let token_hash = common_crypto::hash_sha256(refresh_token);
        
        let db_token = sqlx::query_as!(
            RefreshToken,
            "SELECT * FROM refresh_tokens WHERE token_hash = ? AND revoked = false AND expires_at > ?",
            token_hash,
            Utc::now()
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::TokenExpired)?;

        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL",
            db_token.user_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::UserNotFound)?;

        self.revoke_token(&db_token.id).await?;
        
        let (access_token, new_refresh_token) = self.generate_tokens(&user).await?;

        Ok(RefreshResponse {
            access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.state.config.jwt.access_token_ttl_minutes * 60,
        })
    }

    pub async fn logout(&self, user_id: &str, refresh_token: Option<&str>) -> Result<(), IamError> {
        if let Some(token) = refresh_token {
            let token_hash = common_crypto::hash_sha256(token);
            sqlx::query!(
                "UPDATE refresh_tokens SET revoked = true, revoked_at = ? WHERE token_hash = ?",
                Utc::now(),
                token_hash
            )
            .execute(&self.state.db)
            .await?;
        }

        sqlx::query!(
            "UPDATE refresh_tokens SET revoked = true, revoked_at = ? WHERE user_id = ?",
            Utc::now(),
            user_id
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    pub async fn change_password(&self, user_id: &str, req: ChangePasswordRequest) -> Result<(), IamError> {
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL",
            user_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::UserNotFound)?;

        if !verify_password(&req.old_password, &user.password_hash, &user.salt)? {
            return Err(IamError::InvalidCredentials);
        }

        let (new_hash, new_salt) = hash_password(&req.new_password)?;
        
        sqlx::query!(
            "UPDATE users SET password_hash = ?, salt = ?, password_expired_at = NULL, updated_at = ? WHERE id = ?",
            new_hash,
            new_salt,
            Utc::now(),
            user_id
        )
        .execute(&self.state.db)
        .await?;

        self.logout(user_id, None).await?;

        Ok(())
    }

    async fn generate_tokens(&self, user: &User) -> Result<(String, String), IamError> {
        let principal = Principal {
            user_id: user.id.clone(),
            username: user.username.clone(),
            roles: vec![],
            permissions: vec![],
        };

        let access_token = self.state.token_issuer.issue_access_token(&principal)?;
        
        let refresh_token = nanoid!();
        let token_hash = common_crypto::hash_sha256(&refresh_token);
        let expires_at = Utc::now() + chrono::Duration::days(self.state.config.jwt.refresh_token_ttl_days as i64);

        sqlx::query!(
            "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, revoked) VALUES (?, ?, ?, ?, ?, ?, ?)",
            nanoid!(),
            user.id,
            token_hash,
            expires_at,
            "unknown",
            None::<String>,
            false
        )
        .execute(&self.state.db)
        .await?;

        Ok((access_token, refresh_token))
    }

    async fn revoke_token(&self, token_id: &str) -> Result<(), IamError> {
        sqlx::query!(
            "UPDATE refresh_tokens SET revoked = true, revoked_at = ? WHERE id = ?",
            Utc::now(),
            token_id
        )
        .execute(&self.state.db)
        .await?;
        Ok(())
    }

    async fn update_last_login(&self, user_id: &str) -> Result<(), IamError> {
        sqlx::query!(
            "UPDATE users SET last_login_at = ?, updated_at = ? WHERE id = ?",
            Utc::now(),
            Utc::now(),
            user_id
        )
        .execute(&self.state.db)
        .await?;
        Ok(())
    }

    async fn log_login_success(&self, user: &User) -> Result<(), IamError> {
        let log = LoginLog {
            id: nanoid!(),
            user_id: Some(user.id.clone()),
            username: user.username.clone(),
            ip_address: "unknown".to_string(),
            user_agent: None,
            location: None,
            login_type: LoginType::Password,
            success: true,
            error_message: None,
            created_at: Utc::now(),
        };

        sqlx::query!(
            "INSERT INTO login_logs (id, user_id, username, ip_address, login_type, success, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            log.id,
            log.user_id,
            log.username,
            log.ip_address,
            "password",
            log.success,
            log.created_at
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    async fn log_login_failure(&self, username: &str, error: &str) -> Result<(), IamError> {
        let log = LoginLog {
            id: nanoid!(),
            user_id: None,
            username: username.to_string(),
            ip_address: "unknown".to_string(),
            user_agent: None,
            location: None,
            login_type: LoginType::Password,
            success: false,
            error_message: Some(error.to_string()),
            created_at: Utc::now(),
        };

        sqlx::query!(
            "INSERT INTO login_logs (id, username, ip_address, login_type, success, error_message, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            log.id,
            log.username,
            log.ip_address,
            "password",
            log.success,
            log.error_message,
            log.created_at
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    async fn build_user_info(&self, user: &User) -> Result<UserInfo, IamError> {
        Ok(UserInfo {
            id: user.id.clone(),
            username: user.username.clone(),
            nickname: user.nickname.clone(),
            email: user.email.clone(),
            phone: user.phone.clone(),
            avatar: user.avatar.clone(),
            roles: vec![],
            permissions: vec![],
        })
    }
}
```

- [ ] **Step 3: 创建 service/user_svc.rs**

```rust
use crate::{
    error::IamError,
    models::{User, UserRole},
    state::AppState,
    dto::{CreateUserRequest, UpdateUserRequest, UserQuery, UserResponse, UserListResponse},
};
use common_crypto::password::hash_password;
use chrono::{DateTime, Utc};
use nanoid::nanoid;

pub struct UserService {
    state: AppState,
}

impl UserService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateUserRequest) -> Result<UserResponse, IamError> {
        let exists = sqlx::query!(
            "SELECT id FROM users WHERE username = ? AND deleted_at IS NULL",
            req.username
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if exists {
            return Err(IamError::DuplicateUser);
        }

        let (password_hash, salt) = hash_password(&req.password)?;
        let user_id = nanoid!();

        let user = sqlx::query_as!(
            User,
            "INSERT INTO users (id, username, password_hash, salt, nickname, email, phone, avatar, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?) RETURNING *",
            user_id,
            req.username,
            password_hash,
            salt,
            req.nickname,
            req.email,
            req.phone,
            req.avatar,
            Utc::now(),
            Utc::now()
        )
        .fetch_one(&self.state.db)
        .await?;

        for role_id in req.role_ids {
            sqlx::query!(
                "INSERT INTO user_roles (user_id, role_id, created_at) VALUES (?, ?, ?)",
                user_id,
                role_id,
                Utc::now()
            )
            .execute(&self.state.db)
            .await?;
        }

        Ok(self.to_response(&user))
    }

    pub async fn get(&self, user_id: &str) -> Result<UserResponse, IamError> {
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL",
            user_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::UserNotFound)?;

        Ok(self.to_response(&user))
    }

    pub async fn list(&self, query: UserQuery) -> Result<UserListResponse, IamError> {
        let offset = (query.page - 1) * query.size;
        
        let base_query = "SELECT * FROM users WHERE deleted_at IS NULL";
        let count_query = "SELECT COUNT(*) FROM users WHERE deleted_at IS NULL";
        
        let mut query_str = base_query.to_string();
        let mut count_str = count_query.to_string();
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(keyword) = &query.keyword {
            query_str += " AND (username LIKE ? OR nickname LIKE ? OR email LIKE ?)";
            count_str += " AND (username LIKE ? OR nickname LIKE ? OR email LIKE ?)";
            let like_pattern = format!("%{}%", keyword);
            params.extend(vec![like_pattern.clone().into(); 6]);
        }

        if let Some(status) = &query.status {
            query_str += " AND status = ?";
            count_str += " AND status = ?";
            params.push(status.into());
        }

        query_str += " ORDER BY created_at DESC LIMIT ? OFFSET ?";
        params.push(query.size.into());
        params.push(offset.into());

        let users = sqlx::query_as!(User, &query_str, params[..])
            .fetch_all(&self.state.db)
            .await?;

        let total = sqlx::query_scalar!(count_str, params[..])
            .fetch_one(&self.state.db)
            .await?;

        Ok(UserListResponse {
            items: users.into_iter().map(|u| self.to_response(&u)).collect(),
            total: total.unwrap_or(0) as u64,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(&self, user_id: &str, req: UpdateUserRequest) -> Result<UserResponse, IamError> {
        let mut update_fields = vec![];
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(nickname) = &req.nickname {
            update_fields.push("nickname = ?");
            params.push(nickname.into());
        }
        if let Some(email) = &req.email {
            update_fields.push("email = ?");
            params.push(email.into());
        }
        if let Some(phone) = &req.phone {
            update_fields.push("phone = ?");
            params.push(phone.into());
        }
        if let Some(avatar) = &req.avatar {
            update_fields.push("avatar = ?");
            params.push(avatar.into());
        }
        if let Some(status) = &req.status {
            update_fields.push("status = ?");
            params.push(status.into());
        }

        if update_fields.is_empty() {
            return self.get(user_id);
        }

        update_fields.push("updated_at = ?");
        params.push(Utc::now().into());
        params.push(user_id.into());

        let query_str = format!("UPDATE users SET {} WHERE id = ? AND deleted_at IS NULL RETURNING *", update_fields.join(", "));

        let user = sqlx::query_as!(User, &query_str, params[..])
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::UserNotFound)?;

        Ok(self.to_response(&user))
    }

    pub async fn delete(&self, user_id: &str) -> Result<(), IamError> {
        let result = sqlx::query!(
            "UPDATE users SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            Utc::now(),
            Utc::now(),
            user_id
        )
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::UserNotFound);
        }

        Ok(())
    }

    fn to_response(&self, user: &User) -> UserResponse {
        UserResponse {
            id: user.id.clone(),
            username: user.username.clone(),
            email: user.email.clone(),
            phone: user.phone.clone(),
            nickname: user.nickname.clone(),
            avatar: user.avatar.clone(),
            status: format!("{}", user.status),
            email_verified: user.email_verified,
            phone_verified: user.phone_verified,
            last_login_at: user.last_login_at,
            last_login_ip: user.last_login_ip.clone(),
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}
```

- [ ] **Step 4: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 8: 创建剩余服务层模块

**Files:**
- Create: `services/iam-service/src/service/role_svc.rs`
- Create: `services/iam-service/src/service/permission_svc.rs`
- Create: `services/iam-service/src/service/menu_svc.rs`
- Create: `services/iam-service/src/service/org_svc.rs`
- Create: `services/iam-service/src/service/token_svc.rs`

- [ ] **Step 1: 创建 service/role_svc.rs**

```rust
use crate::{
    error::IamError,
    models::{Role, RolePermission},
    state::AppState,
    dto::{CreateRoleRequest, UpdateRoleRequest, RoleQuery, RoleResponse, RoleListResponse},
};
use chrono::{DateTime, Utc};
use nanoid::nanoid;

pub struct RoleService {
    state: AppState,
}

impl RoleService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateRoleRequest) -> Result<RoleResponse, IamError> {
        let exists = sqlx::query!(
            "SELECT id FROM roles WHERE code = ? AND deleted_at IS NULL",
            req.code
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if exists {
            return Err(IamError::DuplicateRole);
        }

        let role_id = nanoid!();
        let data_scope = match req.data_scope {
            0 => "all",
            1 => "org",
            2 => "dept",
            3 => "self",
            _ => "self",
        };

        let role = sqlx::query_as!(
            Role,
            "INSERT INTO roles (id, name, code, description, data_scope, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'active', ?, ?) RETURNING *",
            role_id,
            req.name,
            req.code,
            req.description,
            data_scope,
            Utc::now(),
            Utc::now()
        )
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&role))
    }

    pub async fn get(&self, role_id: &str) -> Result<RoleResponse, IamError> {
        let role = sqlx::query_as!(
            Role,
            "SELECT * FROM roles WHERE id = ? AND deleted_at IS NULL",
            role_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::RoleNotFound)?;

        Ok(self.to_response(&role))
    }

    pub async fn list(&self, query: RoleQuery) -> Result<RoleListResponse, IamError> {
        let offset = (query.page - 1) * query.size;
        
        let mut query_str = "SELECT * FROM roles WHERE deleted_at IS NULL";
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(keyword) = &query.keyword {
            query_str += " AND (name LIKE ? OR code LIKE ?)";
            let like_pattern = format!("%{}%", keyword);
            params.extend(vec![like_pattern.clone().into(); 2]);
        }

        if let Some(status) = &query.status {
            query_str += " AND status = ?";
            params.push(status.into());
        }

        let count: u64 = sqlx::query_scalar!("SELECT COUNT(*) FROM roles WHERE deleted_at IS NULL", params[..])
            .fetch_one(&self.state.db)
            .await?
            .unwrap_or(0) as u64;

        query_str += " ORDER BY created_at DESC LIMIT ? OFFSET ?";
        params.push(query.size.into());
        params.push(offset.into());

        let roles = sqlx::query_as!(Role, &query_str, params[..])
            .fetch_all(&self.state.db)
            .await?;

        Ok(RoleListResponse {
            items: roles.into_iter().map(|r| self.to_response(&r)).collect(),
            total: count,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(&self, role_id: &str, req: UpdateRoleRequest) -> Result<RoleResponse, IamError> {
        let mut update_fields = vec![];
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(name) = &req.name {
            update_fields.push("name = ?");
            params.push(name.into());
        }
        if let Some(description) = &req.description {
            update_fields.push("description = ?");
            params.push(description.into());
        }
        if let Some(data_scope) = &req.data_scope {
            let scope_str = match data_scope {
                0 => "all",
                1 => "org",
                2 => "dept",
                3 => "self",
                _ => "self",
            };
            update_fields.push("data_scope = ?");
            params.push(scope_str.into());
        }
        if let Some(status) = &req.status {
            update_fields.push("status = ?");
            params.push(status.into());
        }

        if update_fields.is_empty() {
            return self.get(role_id);
        }

        update_fields.push("updated_at = ?");
        params.push(Utc::now().into());
        params.push(role_id.into());

        let query_str = format!("UPDATE roles SET {} WHERE id = ? AND deleted_at IS NULL RETURNING *", update_fields.join(", "));

        let role = sqlx::query_as!(Role, &query_str, params[..])
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::RoleNotFound)?;

        Ok(self.to_response(&role))
    }

    pub async fn delete(&self, role_id: &str) -> Result<(), IamError> {
        let result = sqlx::query!(
            "UPDATE roles SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            Utc::now(),
            Utc::now(),
            role_id
        )
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::RoleNotFound);
        }

        sqlx::query!(
            "DELETE FROM role_permissions WHERE role_id = ?",
            role_id
        )
        .execute(&self.state.db)
        .await?;

        sqlx::query!(
            "DELETE FROM user_roles WHERE role_id = ?",
            role_id
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    pub async fn assign_permissions(&self, role_id: &str, permission_ids: Vec<String>) -> Result<(), IamError> {
        sqlx::query!(
            "DELETE FROM role_permissions WHERE role_id = ?",
            role_id
        )
        .execute(&self.state.db)
        .await?;

        for permission_id in permission_ids {
            sqlx::query!(
                "INSERT INTO role_permissions (role_id, permission_id, created_at) VALUES (?, ?, ?)",
                role_id,
                permission_id,
                Utc::now()
            )
            .execute(&self.state.db)
            .await?;
        }

        Ok(())
    }

    fn to_response(&self, role: &Role) -> RoleResponse {
        RoleResponse {
            id: role.id.clone(),
            name: role.name.clone(),
            code: role.code.clone(),
            description: role.description.clone(),
            data_scope: format!("{}", role.data_scope),
            status: format!("{}", role.status),
            created_at: role.created_at,
            updated_at: role.updated_at,
        }
    }
}
```

- [ ] **Step 2: 创建 service/permission_svc.rs**

```rust
use crate::{
    error::IamError,
    models::{Permission},
    state::AppState,
    dto::{CreatePermissionRequest, UpdatePermissionRequest, PermissionQuery, PermissionResponse, PermissionListResponse},
};
use chrono::{DateTime, Utc};
use nanoid::nanoid;

pub struct PermissionService {
    state: AppState,
}

impl PermissionService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreatePermissionRequest) -> Result<PermissionResponse, IamError> {
        let exists = sqlx::query!(
            "SELECT id FROM permissions WHERE code = ? AND deleted_at IS NULL",
            req.code
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if exists {
            return Err(IamError::DuplicatePermission);
        }

        let permission_id = nanoid!();
        let category = match req.category {
            0 => "menu",
            1 => "button",
            2 => "api",
            _ => "api",
        };

        let permission = sqlx::query_as!(
            Permission,
            "INSERT INTO permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'enabled', ?, ?) RETURNING *",
            permission_id,
            req.code,
            req.name,
            req.description,
            category,
            req.resource,
            req.action,
            req.sort_order.unwrap_or(0),
            Utc::now(),
            Utc::now()
        )
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&permission))
    }

    pub async fn get(&self, permission_id: &str) -> Result<PermissionResponse, IamError> {
        let permission = sqlx::query_as!(
            Permission,
            "SELECT * FROM permissions WHERE id = ? AND deleted_at IS NULL",
            permission_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::PermissionNotFound)?;

        Ok(self.to_response(&permission))
    }

    pub async fn list(&self, query: PermissionQuery) -> Result<PermissionListResponse, IamError> {
        let offset = (query.page - 1) * query.size;
        
        let mut query_str = "SELECT * FROM permissions WHERE deleted_at IS NULL";
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(keyword) = &query.keyword {
            query_str += " AND (code LIKE ? OR name LIKE ?)";
            let like_pattern = format!("%{}%", keyword);
            params.extend(vec![like_pattern.clone().into(); 2]);
        }

        if let Some(category) = &query.category {
            let cat_str = match category {
                0 => "menu",
                1 => "button",
                2 => "api",
                _ => "api",
            };
            query_str += " AND category = ?";
            params.push(cat_str.into());
        }

        let count: u64 = sqlx::query_scalar!("SELECT COUNT(*) FROM permissions WHERE deleted_at IS NULL", params[..])
            .fetch_one(&self.state.db)
            .await?
            .unwrap_or(0) as u64;

        query_str += " ORDER BY sort_order, created_at LIMIT ? OFFSET ?";
        params.push(query.size.into());
        params.push(offset.into());

        let permissions = sqlx::query_as!(Permission, &query_str, params[..])
            .fetch_all(&self.state.db)
            .await?;

        Ok(PermissionListResponse {
            items: permissions.into_iter().map(|p| self.to_response(&p)).collect(),
            total: count,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(&self, permission_id: &str, req: UpdatePermissionRequest) -> Result<PermissionResponse, IamError> {
        let mut update_fields = vec![];
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(name) = &req.name {
            update_fields.push("name = ?");
            params.push(name.into());
        }
        if let Some(description) = &req.description {
            update_fields.push("description = ?");
            params.push(description.into());
        }
        if let Some(sort_order) = &req.sort_order {
            update_fields.push("sort_order = ?");
            params.push(sort_order.into());
        }
        if let Some(status) = &req.status {
            update_fields.push("status = ?");
            params.push(status.into());
        }

        if update_fields.is_empty() {
            return self.get(permission_id);
        }

        update_fields.push("updated_at = ?");
        params.push(Utc::now().into());
        params.push(permission_id.into());

        let query_str = format!("UPDATE permissions SET {} WHERE id = ? AND deleted_at IS NULL RETURNING *", update_fields.join(", "));

        let permission = sqlx::query_as!(Permission, &query_str, params[..])
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::PermissionNotFound)?;

        Ok(self.to_response(&permission))
    }

    pub async fn delete(&self, permission_id: &str) -> Result<(), IamError> {
        let result = sqlx::query!(
            "UPDATE permissions SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            Utc::now(),
            Utc::now(),
            permission_id
        )
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::PermissionNotFound);
        }

        sqlx::query!(
            "DELETE FROM role_permissions WHERE permission_id = ?",
            permission_id
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    fn to_response(&self, permission: &Permission) -> PermissionResponse {
        PermissionResponse {
            id: permission.id.clone(),
            code: permission.code.clone(),
            name: permission.name.clone(),
            description: permission.description.clone(),
            category: format!("{}", permission.category),
            resource: permission.resource.clone(),
            action: permission.action.clone(),
            sort_order: permission.sort_order,
            status: format!("{}", permission.status),
            created_at: permission.created_at,
            updated_at: permission.updated_at,
        }
    }
}
```

- [ ] **Step 3: 创建 service/menu_svc.rs**

```rust
use crate::{
    error::IamError,
    models::{Menu},
    state::AppState,
    dto::{CreateMenuRequest, UpdateMenuRequest, MenuResponse, MenuListResponse},
};
use chrono::{DateTime, Utc};
use nanoid::nanoid;

pub struct MenuService {
    state: AppState,
}

impl MenuService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateMenuRequest) -> Result<MenuResponse, IamError> {
        let exists = sqlx::query!(
            "SELECT id FROM menus WHERE code = ? AND deleted_at IS NULL",
            req.code
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if exists {
            return Err(IamError::DuplicateMenu);
        }

        let menu_id = nanoid!();

        let menu = sqlx::query_as!(
            Menu,
            "INSERT INTO menus (id, parent_id, name, code, path, component, icon, sort_order, visible, status, permission_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'enabled', ?, ?, ?) RETURNING *",
            menu_id,
            req.parent_id,
            req.name,
            req.code,
            req.path,
            req.component,
            req.icon,
            req.sort_order.unwrap_or(0),
            req.visible.unwrap_or(true),
            req.permission_id,
            Utc::now(),
            Utc::now()
        )
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&menu, vec![]))
    }

    pub async fn get(&self, menu_id: &str) -> Result<MenuResponse, IamError> {
        let menu = sqlx::query_as!(
            Menu,
            "SELECT * FROM menus WHERE id = ? AND deleted_at IS NULL",
            menu_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::MenuNotFound)?;

        let children = self.get_children(&menu_id).await?;

        Ok(self.to_response(&menu, children))
    }

    pub async fn list(&self) -> Result<MenuListResponse, IamError> {
        let menus = sqlx::query_as!(
            Menu,
            "SELECT * FROM menus WHERE parent_id IS NULL AND deleted_at IS NULL ORDER BY sort_order"
        )
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for menu in menus {
            let children = self.get_children(&menu.id).await?;
            result.push(self.to_response(&menu, children));
        }

        Ok(MenuListResponse {
            items: result,
            total: result.len() as u64,
        })
    }

    async fn get_children(&self, parent_id: &str) -> Result<Vec<MenuResponse>, IamError> {
        let children = sqlx::query_as!(
            Menu,
            "SELECT * FROM menus WHERE parent_id = ? AND deleted_at IS NULL ORDER BY sort_order",
            parent_id
        )
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for child in children {
            let grandchildren = self.get_children(&child.id).await?;
            result.push(self.to_response(&child, grandchildren));
        }

        Ok(result)
    }

    pub async fn update(&self, menu_id: &str, req: UpdateMenuRequest) -> Result<MenuResponse, IamError> {
        let mut update_fields = vec![];
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(parent_id) = &req.parent_id {
            update_fields.push("parent_id = ?");
            params.push(parent_id.into());
        }
        if let Some(name) = &req.name {
            update_fields.push("name = ?");
            params.push(name.into());
        }
        if let Some(code) = &req.code {
            update_fields.push("code = ?");
            params.push(code.into());
        }
        if let Some(path) = &req.path {
            update_fields.push("path = ?");
            params.push(path.into());
        }
        if let Some(component) = &req.component {
            update_fields.push("component = ?");
            params.push(component.into());
        }
        if let Some(icon) = &req.icon {
            update_fields.push("icon = ?");
            params.push(icon.into());
        }
        if let Some(sort_order) = &req.sort_order {
            update_fields.push("sort_order = ?");
            params.push(sort_order.into());
        }
        if let Some(visible) = &req.visible {
            update_fields.push("visible = ?");
            params.push(visible.into());
        }
        if let Some(permission_id) = &req.permission_id {
            update_fields.push("permission_id = ?");
            params.push(permission_id.into());
        }
        if let Some(status) = &req.status {
            update_fields.push("status = ?");
            params.push(status.into());
        }

        if update_fields.is_empty() {
            return self.get(menu_id);
        }

        update_fields.push("updated_at = ?");
        params.push(Utc::now().into());
        params.push(menu_id.into());

        let query_str = format!("UPDATE menus SET {} WHERE id = ? AND deleted_at IS NULL RETURNING *", update_fields.join(", "));

        let menu = sqlx::query_as!(Menu, &query_str, params[..])
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::MenuNotFound)?;

        Ok(self.to_response(&menu, vec![]))
    }

    pub async fn delete(&self, menu_id: &str) -> Result<(), IamError> {
        let has_children = sqlx::query!(
            "SELECT id FROM menus WHERE parent_id = ? AND deleted_at IS NULL",
            menu_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if has_children {
            return Err(IamError::Conflict("Cannot delete menu with children".to_string()));
        }

        let result = sqlx::query!(
            "UPDATE menus SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            Utc::now(),
            Utc::now(),
            menu_id
        )
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::MenuNotFound);
        }

        Ok(())
    }

    fn to_response(&self, menu: &Menu, children: Vec<MenuResponse>) -> MenuResponse {
        MenuResponse {
            id: menu.id.clone(),
            parent_id: menu.parent_id.clone(),
            name: menu.name.clone(),
            code: menu.code.clone(),
            path: menu.path.clone(),
            component: menu.component.clone(),
            icon: menu.icon.clone(),
            sort_order: menu.sort_order,
            visible: menu.visible,
            status: format!("{}", menu.status),
            permission_id: menu.permission_id.clone(),
            created_at: menu.created_at,
            updated_at: menu.updated_at,
            children,
        }
    }
}
```

- [ ] **Step 4: 创建 service/org_svc.rs**

```rust
use crate::{
    error::IamError,
    models::{OrgUnit, UserOrgUnit},
    state::AppState,
    dto::{CreateOrgUnitRequest, UpdateOrgUnitRequest, OrgUnitResponse, AssignUsersRequest},
};
use chrono::{DateTime, Utc};
use nanoid::nanoid;

pub struct OrgService {
    state: AppState,
}

impl OrgService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateOrgUnitRequest) -> Result<OrgUnitResponse, IamError> {
        let exists = sqlx::query!(
            "SELECT id FROM org_units WHERE code = ? AND deleted_at IS NULL",
            req.code
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if exists {
            return Err(IamError::DuplicateOrgUnit);
        }

        let org_unit_id = nanoid!();
        let level = if req.parent_id.is_some() {
            let parent = sqlx::query_as!(
                OrgUnit,
                "SELECT level FROM org_units WHERE id = ? AND deleted_at IS NULL",
                req.parent_id.as_ref().unwrap()
            )
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::OrgUnitNotFound)?;
            parent.level + 1
        } else {
            1
        };

        let org_unit = sqlx::query_as!(
            OrgUnit,
            "INSERT INTO org_units (id, parent_id, name, code, level, sort_order, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?) RETURNING *",
            org_unit_id,
            req.parent_id,
            req.name,
            req.code,
            level,
            req.sort_order.unwrap_or(0),
            Utc::now(),
            Utc::now()
        )
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn get(&self, org_unit_id: &str) -> Result<OrgUnitResponse, IamError> {
        let org_unit = sqlx::query_as!(
            OrgUnit,
            "SELECT * FROM org_units WHERE id = ? AND deleted_at IS NULL",
            org_unit_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::OrgUnitNotFound)?;

        let children = self.get_children(&org_unit_id).await?;

        Ok(self.to_response(&org_unit, children))
    }

    pub async fn list(&self) -> Result<Vec<OrgUnitResponse>, IamError> {
        let org_units = sqlx::query_as!(
            OrgUnit,
            "SELECT * FROM org_units WHERE parent_id IS NULL AND deleted_at IS NULL ORDER BY sort_order"
        )
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for org_unit in org_units {
            let children = self.get_children(&org_unit.id).await?;
            result.push(self.to_response(&org_unit, children));
        }

        Ok(result)
    }

    async fn get_children(&self, parent_id: &str) -> Result<Vec<OrgUnitResponse>, IamError> {
        let children = sqlx::query_as!(
            OrgUnit,
            "SELECT * FROM org_units WHERE parent_id = ? AND deleted_at IS NULL ORDER BY sort_order",
            parent_id
        )
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for child in children {
            let grandchildren = self.get_children(&child.id).await?;
            result.push(self.to_response(&child, grandchildren));
        }

        Ok(result)
    }

    pub async fn update(&self, org_unit_id: &str, req: UpdateOrgUnitRequest) -> Result<OrgUnitResponse, IamError> {
        let mut update_fields = vec![];
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(parent_id) = &req.parent_id {
            update_fields.push("parent_id = ?");
            params.push(parent_id.into());
        }
        if let Some(name) = &req.name {
            update_fields.push("name = ?");
            params.push(name.into());
        }
        if let Some(code) = &req.code {
            update_fields.push("code = ?");
            params.push(code.into());
        }
        if let Some(sort_order) = &req.sort_order {
            update_fields.push("sort_order = ?");
            params.push(sort_order.into());
        }
        if let Some(status) = &req.status {
            update_fields.push("status = ?");
            params.push(status.into());
        }

        if update_fields.is_empty() {
            return self.get(org_unit_id);
        }

        update_fields.push("updated_at = ?");
        params.push(Utc::now().into());
        params.push(org_unit_id.into());

        let query_str = format!("UPDATE org_units SET {} WHERE id = ? AND deleted_at IS NULL RETURNING *", update_fields.join(", "));

        let org_unit = sqlx::query_as!(OrgUnit, &query_str, params[..])
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::OrgUnitNotFound)?;

        Ok(self.to_response(&org_unit, vec![]))
    }

    pub async fn delete(&self, org_unit_id: &str) -> Result<(), IamError> {
        let has_children = sqlx::query!(
            "SELECT id FROM org_units WHERE parent_id = ? AND deleted_at IS NULL",
            org_unit_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if has_children {
            return Err(IamError::Conflict("Cannot delete org unit with children".to_string()));
        }

        let result = sqlx::query!(
            "UPDATE org_units SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            Utc::now(),
            Utc::now(),
            org_unit_id
        )
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::OrgUnitNotFound);
        }

        sqlx::query!(
            "DELETE FROM user_org_units WHERE org_unit_id = ?",
            org_unit_id
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    pub async fn assign_users(&self, org_unit_id: &str, req: AssignUsersRequest) -> Result<(), IamError> {
        sqlx::query!(
            "DELETE FROM user_org_units WHERE org_unit_id = ?",
            org_unit_id
        )
        .execute(&self.state.db)
        .await?;

        for user_id in req.user_ids {
            sqlx::query!(
                "INSERT INTO user_org_units (user_id, org_unit_id, is_primary, created_at) VALUES (?, ?, ?, ?)",
                user_id,
                org_unit_id,
                false,
                Utc::now()
            )
            .execute(&self.state.db)
            .await?;
        }

        Ok(())
    }

    fn to_response(&self, org_unit: &OrgUnit, children: Vec<OrgUnitResponse>) -> OrgUnitResponse {
        OrgUnitResponse {
            id: org_unit.id.clone(),
            parent_id: org_unit.parent_id.clone(),
            name: org_unit.name.clone(),
            code: org_unit.code.clone(),
            level: org_unit.level,
            sort_order: org_unit.sort_order,
            status: format!("{}", org_unit.status),
            created_at: org_unit.created_at,
            updated_at: org_unit.updated_at,
            children,
        }
    }
}
```

- [ ] **Step 5: 创建 service/token_svc.rs**

```rust
use crate::{
    config::Config,
    error::IamError,
    state::AppState,
};
use jwt_simple::{algorithms::RS256KeyPair, claims::Claims, Authorization, JWTClaims};
use chrono::{DateTime, Utc};

pub struct TokenService {
    state: AppState,
    key_pair: RS256KeyPair,
}

impl TokenService {
    pub fn new(state: AppState) -> Result<Self, IamError> {
        let key_pair = Self::load_key_pair(&state.config.jwt)?;
        Ok(Self { state, key_pair })
    }

    fn load_key_pair(config: &crate::config::JwtConfig) -> Result<RS256KeyPair, IamError> {
        let private_key_pem = std::fs::read_to_string(&config.private_key_path)
            .map_err(|e| IamError::Jwt(jwt_simple::Error::Other(e.to_string())))?;
        
        let key_pair = RS256KeyPair::from_pem(&private_key_pem)
            .map_err(|e| IamError::Jwt(e))?;
        
        Ok(key_pair)
    }

    pub fn issue_access_token(&self, user_id: &str, username: &str, roles: Vec<String>, permissions: Vec<String>) -> Result<String, IamError> {
        let expires_at = Utc::now() + chrono::Duration::minutes(self.state.config.jwt.access_token_ttl_minutes as i64);
        
        let claims = Claims {
            subject: Some(user_id.to_string()),
            issuer: Some("iam-service".to_string()),
            expires_at: Some(Authorization::DateTime(expires_at.into())),
            custom: serde_json::json!({
                "username": username,
                "roles": roles,
                "permissions": permissions,
            }),
            ..Claims::default()
        };

        let token = self.key_pair.sign(claims)
            .map_err(|e| IamError::Jwt(e))?;
        
        Ok(token)
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, IamError> {
        let claims = self.key_pair.verify_token::<Claims>(token, None)
            .map_err(|e| IamError::Jwt(e))?;
        
        Ok(claims)
    }
}
```

- [ ] **Step 6: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 9: 创建路由模块

**Files:**
- Create: `services/iam-service/src/routes/mod.rs`
- Create: `services/iam-service/src/routes/auth.rs`
- Create: `services/iam-service/src/routes/users.rs`
- Create: `services/iam-service/src/routes/roles.rs`
- Create: `services/iam-service/src/routes/permissions.rs`
- Create: `services/iam-service/src/routes/menus.rs`
- Create: `services/iam-service/src/routes/org_units.rs`
- Create: `services/iam-service/src/routes/sessions.rs`

- [ ] **Step 1: 创建 routes/mod.rs**

```rust
pub mod auth;
pub mod menus;
pub mod org_units;
pub mod permissions;
pub mod roles;
pub mod sessions;
pub mod users;

use axum::{Router, routing::get};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(auth::routes(state.clone()))
        .merge(users::routes(state.clone()))
        .merge(roles::routes(state.clone()))
        .merge(permissions::routes(state.clone()))
        .merge(menus::routes(state.clone()))
        .merge(org_units::routes(state.clone()))
        .merge(sessions::routes(state))
        .route("/health", get(health))
}

async fn health() -> &'static str {
    "OK"
}
```

- [ ] **Step 2: 创建 routes/auth.rs**

```rust
use axum::{Router, routing::{post, delete}, extract::{Json, Path, State}};
use crate::{state::AppState, dto::{LoginRequest, RefreshRequest, ChangePasswordRequest, LoginResponse, RefreshResponse}, service::AuthService};
use jwt_ctx::Principal;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/auth/login", post(login))
        .route("/iam/auth/refresh", post(refresh))
        .route("/iam/auth/logout", delete(logout))
        .route("/iam/auth/password", post(change_password))
        .with_state(state)
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, axum::http::StatusCode> {
    let service = AuthService::new(state);
    let response = service.login(req).await.map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;
    Ok(Json(response))
}

async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, axum::http::StatusCode> {
    let service = AuthService::new(state);
    let response = service.refresh(&req.refresh_token).await.map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;
    Ok(Json(response))
}

async fn logout(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<(), axum::http::StatusCode> {
    let service = AuthService::new(state);
    service.logout(&principal.user_id, None).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn change_password(
    State(state): State<AppState>,
    principal: Principal,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<(), axum::http::StatusCode> {
    let service = AuthService::new(state);
    service.change_password(&principal.user_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::InvalidCredentials => axum::http::StatusCode::UNAUTHORIZED,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}
```

- [ ] **Step 3: 创建 routes/users.rs**

```rust
use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, Query, State}};
use crate::{state::AppState, dto::{CreateUserRequest, UpdateUserRequest, UserQuery, UserResponse, UserListResponse}, service::UserService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/users", get(list_users).post(create_user))
        .route("/iam/users/:user_id", get(get_user).put(update_user).delete(delete_user))
        .with_state(state)
}

async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UserQuery>,
) -> Result<Json<UserListResponse>, axum::http::StatusCode> {
    let service = UserService::new(state);
    let response = service.list(query).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, axum::http::StatusCode> {
    let service = UserService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateUser => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserResponse>, axum::http::StatusCode> {
    let service = UserService::new(state);
    let response = service.get(&user_id).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, axum::http::StatusCode> {
    let service = UserService::new(state);
    let response = service.update(&user_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = UserService::new(state);
    service.delete(&user_id).await.map_err(|e| {
        match e {
            crate::error::IamError::UserNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}
```

- [ ] **Step 4: 创建 routes/roles.rs**

```rust
use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, Query, State}};
use crate::{state::AppState, dto::{CreateRoleRequest, UpdateRoleRequest, RoleQuery, RoleResponse, RoleListResponse, AssignPermissionsRequest}, service::RoleService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/roles", get(list_roles).post(create_role))
        .route("/iam/roles/:role_id", get(get_role).put(update_role).delete(delete_role))
        .route("/iam/roles/:role_id/permissions", put(assign_permissions))
        .with_state(state)
}

async fn list_roles(
    State(state): State<AppState>,
    Query(query): Query<RoleQuery>,
) -> Result<Json<RoleListResponse>, axum::http::StatusCode> {
    let service = RoleService::new(state);
    let response = service.list(query).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_role(
    State(state): State<AppState>,
    Json(req): Json<CreateRoleRequest>,
) -> Result<Json<RoleResponse>, axum::http::StatusCode> {
    let service = RoleService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateRole => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
) -> Result<Json<RoleResponse>, axum::http::StatusCode> {
    let service = RoleService::new(state);
    let response = service.get(&role_id).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, axum::http::StatusCode> {
    let service = RoleService::new(state);
    let response = service.update(&role_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_role(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = RoleService::new(state);
    service.delete(&role_id).await.map_err(|e| {
        match e {
            crate::error::IamError::RoleNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}

async fn assign_permissions(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Json(req): Json<AssignPermissionsRequest>,
) -> Result<(), axum::http::StatusCode> {
    let service = RoleService::new(state);
    service.assign_permissions(&role_id, req.permission_ids).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
```

- [ ] **Step 5: 创建 routes/permissions.rs**

```rust
use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, Query, State}};
use crate::{state::AppState, dto::{CreatePermissionRequest, UpdatePermissionRequest, PermissionQuery, PermissionResponse, PermissionListResponse}, service::PermissionService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/permissions", get(list_permissions).post(create_permission))
        .route("/iam/permissions/:permission_id", get(get_permission).put(update_permission).delete(delete_permission))
        .with_state(state)
}

async fn list_permissions(
    State(state): State<AppState>,
    Query(query): Query<PermissionQuery>,
) -> Result<Json<PermissionListResponse>, axum::http::StatusCode> {
    let service = PermissionService::new(state);
    let response = service.list(query).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_permission(
    State(state): State<AppState>,
    Json(req): Json<CreatePermissionRequest>,
) -> Result<Json<PermissionResponse>, axum::http::StatusCode> {
    let service = PermissionService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicatePermission => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_permission(
    State(state): State<AppState>,
    Path(permission_id): Path<String>,
) -> Result<Json<PermissionResponse>, axum::http::StatusCode> {
    let service = PermissionService::new(state);
    let response = service.get(&permission_id).await.map_err(|e| {
        match e {
            crate::error::IamError::PermissionNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_permission(
    State(state): State<AppState>,
    Path(permission_id): Path<String>,
    Json(req): Json<UpdatePermissionRequest>,
) -> Result<Json<PermissionResponse>, axum::http::StatusCode> {
    let service = PermissionService::new(state);
    let response = service.update(&permission_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::PermissionNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_permission(
    State(state): State<AppState>,
    Path(permission_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = PermissionService::new(state);
    service.delete(&permission_id).await.map_err(|e| {
        match e {
            crate::error::IamError::PermissionNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}
```

- [ ] **Step 6: 创建 routes/menus.rs**

```rust
use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, State}};
use crate::{state::AppState, dto::{CreateMenuRequest, UpdateMenuRequest, MenuResponse, MenuListResponse}, service::MenuService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/menus", get(list_menus).post(create_menu))
        .route("/iam/menus/:menu_id", get(get_menu).put(update_menu).delete(delete_menu))
        .with_state(state)
}

async fn list_menus(
    State(state): State<AppState>,
) -> Result<Json<MenuListResponse>, axum::http::StatusCode> {
    let service = MenuService::new(state);
    let response = service.list().await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_menu(
    State(state): State<AppState>,
    Json(req): Json<CreateMenuRequest>,
) -> Result<Json<MenuResponse>, axum::http::StatusCode> {
    let service = MenuService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateMenu => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
) -> Result<Json<MenuResponse>, axum::http::StatusCode> {
    let service = MenuService::new(state);
    let response = service.get(&menu_id).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
    Json(req): Json<UpdateMenuRequest>,
) -> Result<Json<MenuResponse>, axum::http::StatusCode> {
    let service = MenuService::new(state);
    let response = service.update(&menu_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = MenuService::new(state);
    service.delete(&menu_id).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => axum::http::StatusCode::NOT_FOUND,
            crate::error::IamError::Conflict(_) => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}
```

- [ ] **Step 7: 创建 routes/org_units.rs**

```rust
use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, State}};
use crate::{state::AppState, dto::{CreateOrgUnitRequest, UpdateOrgUnitRequest, OrgUnitResponse, AssignUsersRequest}, service::OrgService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/org-units", get(list_org_units).post(create_org_unit))
        .route("/iam/org-units/:org_unit_id", get(get_org_unit).put(update_org_unit).delete(delete_org_unit))
        .route("/iam/org-units/:org_unit_id/users", put(assign_users))
        .with_state(state)
}

async fn list_org_units(
    State(state): State<AppState>,
) -> Result<Json<Vec<OrgUnitResponse>>, axum::http::StatusCode> {
    let service = OrgService::new(state);
    let response = service.list().await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_org_unit(
    State(state): State<AppState>,
    Json(req): Json<CreateOrgUnitRequest>,
) -> Result<Json<OrgUnitResponse>, axum::http::StatusCode> {
    let service = OrgService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateOrgUnit => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
) -> Result<Json<OrgUnitResponse>, axum::http::StatusCode> {
    let service = OrgService::new(state);
    let response = service.get(&org_unit_id).await.map_err(|e| {
        match e {
            crate::error::IamError::OrgUnitNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
    Json(req): Json<UpdateOrgUnitRequest>,
) -> Result<Json<OrgUnitResponse>, axum::http::StatusCode> {
    let service = OrgService::new(state);
    let response = service.update(&org_unit_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::OrgUnitNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = OrgService::new(state);
    service.delete(&org_unit_id).await.map_err(|e| {
        match e {
            crate::error::IamError::OrgUnitNotFound => axum::http::StatusCode::NOT_FOUND,
            crate::error::IamError::Conflict(_) => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}

async fn assign_users(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
    Json(req): Json<AssignUsersRequest>,
) -> Result<(), axum::http::StatusCode> {
    let service = OrgService::new(state);
    service.assign_users(&org_unit_id, req).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
```

- [ ] **Step 8: 创建 routes/sessions.rs**

```rust
use axum::{Router, routing::{get, delete}, extract::{Path, State}};
use crate::{state::AppState, dto::SessionInfo, service::AuthService};
use jwt_ctx::Principal;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/sessions", get(list_sessions))
        .route("/iam/sessions/:session_id", delete(force_logout))
        .with_state(state)
}

async fn list_sessions(
    State(state): State<AppState>,
    _principal: Principal,
) -> Result<Json<Vec<SessionInfo>>, axum::http::StatusCode> {
    Ok(Json(vec![]))
}

async fn force_logout(
    State(state): State<AppState>,
    _principal: Principal,
    Path(session_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    Ok(())
}
```

- [ ] **Step 9: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

---

## Task 10: 创建数据库迁移和测试

**Files:**
- Create: `services/iam-service/migrations/mysql/V001__init.sql`
- Create: `services/iam-service/migrations/sqlite/V001__init.sql`
- Create: `services/iam-service/tests/integration_test.rs`
- Create: `services/iam-service/Dockerfile`

- [ ] **Step 1: 创建 MySQL 迁移文件**

```sql
CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(36) PRIMARY KEY,
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255),
    phone VARCHAR(20),
    password_hash VARCHAR(255) NOT NULL,
    salt VARCHAR(64) NOT NULL,
    nickname VARCHAR(100) NOT NULL,
    avatar VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    phone_verified BOOLEAN NOT NULL DEFAULT FALSE,
    last_login_at DATETIME,
    last_login_ip VARCHAR(50),
    password_expired_at DATETIME,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME
);

CREATE TABLE IF NOT EXISTS roles (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL UNIQUE,
    description TEXT,
    data_scope VARCHAR(20) NOT NULL DEFAULT 'self',
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME
);

CREATE TABLE IF NOT EXISTS permissions (
    id VARCHAR(36) PRIMARY KEY,
    code VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    category VARCHAR(20) NOT NULL,
    resource VARCHAR(255),
    action VARCHAR(50),
    sort_order INT NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'enabled',
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME
);

CREATE TABLE IF NOT EXISTS menus (
    id VARCHAR(36) PRIMARY KEY,
    parent_id VARCHAR(36),
    name VARCHAR(100) NOT NULL,
    code VARCHAR(100) NOT NULL UNIQUE,
    path VARCHAR(255) NOT NULL,
    component VARCHAR(255),
    icon VARCHAR(100),
    sort_order INT NOT NULL DEFAULT 0,
    visible BOOLEAN NOT NULL DEFAULT TRUE,
    status VARCHAR(20) NOT NULL DEFAULT 'enabled',
    permission_id VARCHAR(36),
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME,
    FOREIGN KEY (parent_id) REFERENCES menus(id),
    FOREIGN KEY (permission_id) REFERENCES permissions(id)
);

CREATE TABLE IF NOT EXISTS org_units (
    id VARCHAR(36) PRIMARY KEY,
    parent_id VARCHAR(36),
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL UNIQUE,
    level INT NOT NULL DEFAULT 1,
    sort_order INT NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME,
    FOREIGN KEY (parent_id) REFERENCES org_units(id)
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id VARCHAR(36) NOT NULL,
    role_id VARCHAR(36) NOT NULL,
    created_at DATETIME NOT NULL,
    PRIMARY KEY (user_id, role_id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (role_id) REFERENCES roles(id)
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id VARCHAR(36) NOT NULL,
    permission_id VARCHAR(36) NOT NULL,
    created_at DATETIME NOT NULL,
    PRIMARY KEY (role_id, permission_id),
    FOREIGN KEY (role_id) REFERENCES roles(id),
    FOREIGN KEY (permission_id) REFERENCES permissions(id)
);

CREATE TABLE IF NOT EXISTS user_org_units (
    user_id VARCHAR(36) NOT NULL,
    org_unit_id VARCHAR(36) NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME NOT NULL,
    PRIMARY KEY (user_id, org_unit_id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (org_unit_id) REFERENCES org_units(id)
);

CREATE TABLE IF NOT EXISTS refresh_tokens (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36) NOT NULL,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at DATETIME NOT NULL,
    client_ip VARCHAR(50) NOT NULL,
    user_agent TEXT,
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    revoked_at DATETIME,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS login_logs (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36),
    username VARCHAR(50) NOT NULL,
    ip_address VARCHAR(50) NOT NULL,
    user_agent TEXT,
    location VARCHAR(100),
    login_type VARCHAR(20) NOT NULL,
    success BOOLEAN NOT NULL,
    error_message TEXT,
    created_at DATETIME NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_status ON users(status);
CREATE INDEX idx_roles_code ON roles(code);
CREATE INDEX idx_permissions_code ON permissions(code);
CREATE INDEX idx_refresh_tokens_token_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_login_logs_username ON login_logs(username);
CREATE INDEX idx_login_logs_created_at ON login_logs(created_at);
```

- [ ] **Step 2: 创建 SQLite 迁移文件**

```sql
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    email TEXT,
    phone TEXT,
    password_hash TEXT NOT NULL,
    salt TEXT NOT NULL,
    nickname TEXT NOT NULL,
    avatar TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    email_verified INTEGER NOT NULL DEFAULT 0,
    phone_verified INTEGER NOT NULL DEFAULT 0,
    last_login_at TEXT,
    last_login_ip TEXT,
    password_expired_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    description TEXT,
    data_scope TEXT NOT NULL DEFAULT 'self',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS permissions (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,
    resource TEXT,
    action TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'enabled',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS menus (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL,
    component TEXT,
    icon TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    visible INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'enabled',
    permission_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS org_units (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    level INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, role_id)
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id TEXT NOT NULL,
    permission_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE IF NOT EXISTS user_org_units (
    user_id TEXT NOT NULL,
    org_unit_id TEXT NOT NULL,
    is_primary INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, org_unit_id)
);

CREATE TABLE IF NOT EXISTS refresh_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    client_ip TEXT NOT NULL,
    user_agent TEXT,
    revoked INTEGER NOT NULL DEFAULT 0,
    revoked_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS login_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    username TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    user_agent TEXT,
    location TEXT,
    login_type TEXT NOT NULL,
    success INTEGER NOT NULL,
    error_message TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_status ON users(status);
CREATE INDEX idx_roles_code ON roles(code);
CREATE INDEX idx_permissions_code ON permissions(code);
CREATE INDEX idx_refresh_tokens_token_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_login_logs_username ON login_logs(username);
CREATE INDEX idx_login_logs_created_at ON login_logs(created_at);
```

- [ ] **Step 3: 创建集成测试**

```rust
use common_db::test_kit::sqlite_fixture::SqliteFixture;
use iam_service::{config::load_config, state::AppState};
use reqwest::Client;

#[tokio::test]
async fn test_user_create_and_login() {
    let fixture = SqliteFixture::new().await;
    let pool = fixture.pool();

    let config = load_config().unwrap();
    let state = AppState::new(pool, config.clone());

    let client = Client::new();
    let response = client
        .post("http://localhost:3002/iam/users")
        .json(&serde_json::json!({
            "username": "testuser",
            "password": "password123",
            "nickname": "Test User",
            "email": "test@example.com"
        }))
        .send()
        .await;

    assert!(response.is_ok());
}
```

- [ ] **Step 4: 创建 Dockerfile**

```dockerfile
FROM rust:1.74 as builder

WORKDIR /app

COPY . .

RUN cargo build --release --manifest-path services/iam-service/Cargo.toml

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/iam-service .
COPY --from=builder /app/services/iam-service/config ./config
COPY --from=builder /app/services/iam-service/keys ./keys

EXPOSE 3002

CMD ["./iam-service"]
```

---

## Task 11: 更新路由配置

**Files:**
- Modify: `docker/config/api-gateway/routes/iam.router.toml`

- [ ] **Step 1: 创建 iam.router.toml**

```toml
[[routes]]
name = "iam-auth"
match = "/iam/auth/*"
rewrite = [
  { type = "strip_prefix", prefix = "/iam" }
]
backend = "iam-service"

[[routes]]
name = "iam-users"
match = "/iam/users/*"
rewrite = [
  { type = "strip_prefix", prefix = "/iam" }
]
backend = "iam-service"

[[routes]]
name = "iam-roles"
match = "/iam/roles/*"
rewrite = [
  { type = "strip_prefix", prefix = "/iam" }
]
backend = "iam-service"

[[routes]]
name = "iam-permissions"
match = "/iam/permissions/*"
rewrite = [
  { type = "strip_prefix", prefix = "/iam" }
]
backend = "iam-service"

[[routes]]
name = "iam-menus"
match = "/iam/menus/*"
rewrite = [
  { type = "strip_prefix", prefix = "/iam" }
]
backend = "iam-service"

[[routes]]
name = "iam-org-units"
match = "/iam/org-units/*"
rewrite = [
  { type = "strip_prefix", prefix = "/iam" }
]
backend = "iam-service"

[[routes]]
name = "iam-sessions"
match = "/iam/sessions/*"
rewrite = [
  { type = "strip_prefix", prefix = "/iam" }
]
backend = "iam-service"
```

---

## Task 12: 清理旧服务

**Files:**
- Delete: `services/token-service/` (保留内核能力到 iam-service)
- Delete: `services/credential-login-service/`
- Update: `services/user-service/` (标记为 deprecated)

- [ ] **Step 1: 标记 user-service 为 deprecated**

创建 `services/user-service/DEPRECATED.md`:

```markdown
# User Service - Deprecated

This service has been merged into iam-service.

**Migration Guide:**

| Endpoint | New Endpoint in iam-service |
|----------|-----------------------------|
| /users/* | /iam/users/* |
| /roles/* | /iam/roles/* |
| /permissions/* | /iam/permissions/* |
| /org-units/* | /iam/org-units/* |

**Deprecation Date:** 2026-06-13

**Removal Date:** 2026-09-01
```

- [ ] **Step 2: 更新 Cargo.toml**

Run: `cargo update`
Expected: 更新依赖

---

## Task 13: 验证和测试

**Files:**
- Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
- Run: `cargo test --manifest-path services/iam-service/Cargo.toml`

- [ ] **Step 1: 构建验证**

Run: `cargo build --manifest-path services/iam-service/Cargo.toml`
Expected: 编译成功

- [ ] **Step 2: 运行测试**

Run: `cargo test --manifest-path services/iam-service/Cargo.toml`
Expected: 测试通过

- [ ] **Step 3: 验证路由配置**

Run: `cargo run -p api-gateway -- validate --routes docker/config/api-gateway/routes`
Expected: 路由配置验证通过

---

## 执行选项

Plan complete and saved to `docs/superpowers/plans/2026-06-13-iam-service-merge-plan.md`.

**1. Subagent-Driven (recommended)** - 每个任务派发独立子代理，任务间审核，快速迭代

**2. Inline Execution** - 使用 executing-plans 在当前会话执行，批量执行带检查点

**选择哪种方式？**