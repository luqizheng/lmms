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

    pub async fn assign_permissions(&self, role_id: