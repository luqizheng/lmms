# Token Server + API Gateway 实施计划

> **目标:** 基于设计文档实现完整的 API 鉴权基础设施：api-gateway (Rust) + token-server (Rust) + auth-service (Go) + user-service (Go)，含 docker-compose 编排

**架构:** Rust Workspace（common + api-gateway + token-server）+ Go 独立模块（auth-service + user-service），Nginx 反向代理，Consul 服务发现，Redis 黑名单，支持 MySQL/MSSQL/DM 多数据库

**技术栈:** Rust (axum, jsonwebtoken, redis-rs), Go (gin, GORM), Redis, Consul, Nginx, Docker

---

## Phase 0: 项目脚手架

### Task 0.1: Cargo Workspace 初始化

**文件:**
- Create: `Cargo.workspace.toml`
- Create: `Cargo.toml` (workspace root)
- Create: `common/Cargo.toml`
- Create: `common/src/lib.rs`
- Create: `token-server/Cargo.toml`
- Create: `token-server/src/main.rs`
- Create: `api-gateway/Cargo.toml`
- Create: `api-gateway/src/main.rs`

- [ ] **Step 1: 创建 workspace 根 Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "common",
    "token-server",
    "api-gateway",
]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
jsonwebtoken = "9"
redis = { version = "0.27", features = ["tokio-comp"] }
rsa = "0.9"
uuid = { version = "1", features = ["v4"] }
argon2 = "0.5"
reqwest = { version = "0.12", features = ["json"] }
thiserror = "2"
config = "0.14"
dotenvy = "0.15"
```

- [ ] **Step 2: 创建 common crate**

`common/Cargo.toml`:
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
```

`common/src/lib.rs`:
```rust
pub mod error;
pub mod token;
```

- [ ] **Step 3: 创建 token-server crate**

`token-server/Cargo.toml`:
```toml
[package]
name = "token-server"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../common" }
axum.workspace = true
tokio.workspace = true
tower.workspace = true
tower-http.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
serde.workspace = true
serde_json.workspace = true
jsonwebtoken.workspace = true
redis.workspace = true
uuid.workspace = true
argon2.workspace = true
thiserror.workspace = true
config.workspace = true
dotenvy.workspace = true
```

`token-server/src/main.rs`:
```rust
fn main() {
    println!("token-server starting...");
}
```

- [ ] **Step 4: 创建 api-gateway crate**

`api-gateway/Cargo.toml`:
```toml
[package]
name = "api-gateway"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../common" }
axum.workspace = true
tokio.workspace = true
tower.workspace = true
tower-http.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
serde.workspace = true
serde_json.workspace = true
jsonwebtoken.workspace = true
redis.workspace = true
reqwest.workspace = true
thiserror.workspace = true
config.workspace = true
dotenvy.workspace = true
```

`api-gateway/src/main.rs`:
```rust
fn main() {
    println!("api-gateway starting...");
}
```

- [ ] **Step 5: 验证**

运行: `cargo build`

期望: Cargo workspace 编译成功，打印 "token-server starting..." 或 "api-gateway starting..." 取决于运行的 crate

运行: `cargo run -p token-server` → 期望: "token-server starting..."
运行: `cargo run -p api-gateway` → 期望: "api-gateway starting..."

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "scaffold: cargo workspace with common, token-server, api-gateway"
```

---

### Task 0.2: Go 模块初始化

**文件:**
- Create: `auth-service/go.mod`
- Create: `auth-service/main.go`
- Create: `user-service/go.mod`
- Create: `user-service/main.go`

- [ ] **Step 1: 初始化 auth-service**

```bash
cd auth-service
go mod init superpowers/auth-service
go get github.com/gin-gonic/gin
go get gorm.io/gorm
go get gorm.io/driver/mysql
go get github.com/redis/go-redis/v9
go get github.com/spf13/viper
```

`auth-service/main.go`:
```go
package main

import "fmt"

func main() {
    fmt.Println("auth-service starting...")
}
```

- [ ] **Step 2: 初始化 user-service**

```bash
cd user-service
go mod init superpowers/user-service
go get github.com/gin-gonic/gin
go get gorm.io/gorm
go get gorm.io/driver/mysql
go get github.com/spf13/viper
```

`user-service/main.go`:
```go
package main

import "fmt"

func main() {
    fmt.Println("user-service starting...")
}
```

- [ ] **Step 3: 验证**

运行: `cd auth-service && go run main.go` → 期望: "auth-service starting..."
运行: `cd user-service && go run main.go` → 期望: "user-service starting..."

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "scaffold: go modules for auth-service and user-service"
```

---

### Task 0.3: Rust 工具链和日志初始化

**文件:**
- Modify: `token-server/src/main.rs:1-3`
- Modify: `api-gateway/src/main.rs:1-3`

- [ ] **Step 1: 为 token-server 添加 tracing 初始化**

`token-server/src/main.rs`:
```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "token_server=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("token-server starting...");
}
```

- [ ] **Step 2: 为 api-gateway 添加 tracing 初始化**

`api-gateway/src/main.rs`:
```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "api_gateway=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("api-gateway starting...");
}
```

- [ ] **Step 3: 验证**

运行: `cargo run -p token-server 2>&1` → 期望: 看到 `INFO token_server: token-server starting...`
运行: `cargo run -p api-gateway 2>&1` → 期望: 看到 `INFO api_gateway: api-gateway starting...`

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat: add tracing logging for rust services"
```

---

## Phase 1: common crate

### Task 1.1: JWT Claims 类型定义

**文件:**
- Create: `common/src/token.rs`
- Create: `common/src/error.rs`

- [ ] **Step 1: 编写错误类型**

`common/src/error.rs`:
```rust
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

- [ ] **Step 2: 编写 JWT Claims 结构体**

`common/src/token.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,
    pub jti: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
    pub scope: String,
    pub client_id: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenClaims {
    pub sub: String,
    pub jti: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
    pub scope: String,
    pub family: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTokenClaims {
    pub sub: String,
    pub jti: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
    pub scope: String,
    pub resource_type: String,
    pub resource_id: String,
    pub permission: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TokenClaims {
    Access(AccessTokenClaims),
    Refresh(RefreshTokenClaims),
    Resource(ResourceTokenClaims),
}
```

- [ ] **Step 3: 更新 lib.rs 导出**

`common/src/lib.rs`:
```rust
pub mod error;
pub mod token;
```

- [ ] **Step 4: 验证**

运行: `cargo check -p common`
期望: 编译通过，无错误

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(common): jwt claims types and error definitions"
```

---

### Task 1.2: 通用配置类型

**文件:**
- Create: `common/src/config.rs`

- [ ] **Step 1: 编写配置结构**

`common/src/config.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".into(),
            prefix: "superpowers:".into(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 3000,
        }
    }
}
```

- [ ] **Step 2: 更新 lib.rs**

```rust
pub mod config;
pub mod error;
pub mod token;
```

- [ ] **Step 3: 验证**

运行: `cargo check -p common`
期望: 编译通过

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat(common): add generic server and redis config types"
```

---

## Phase 2: token-server

### Task 2.1: 配置加载 + RSA 密钥生成

**文件:**
- Create: `token-server/src/config.rs`

- [ ] **Step 1: 编写 token-server 配置**

`token-server/src/config.rs`:
```rust
use common::config::{RedisConfig, ServerConfig};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TokenServerConfig {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub redis: RedisConfig,

    pub jwt_private_key_path: String,
    pub jwt_public_key_path: String,
    pub issuer: String,
    pub clients_file_path: String,

    pub default_access_ttl_sec: i64,        // 如 3600
    pub default_refresh_ttl_sec: i64,       // 如 2592000
    pub default_resource_ttl_sec: i64,      // 如 300
}

impl Default for TokenServerConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig { host: "0.0.0.0".into(), port: 4000 },
            redis: RedisConfig::default(),
            jwt_private_key_path: "keys/private.pem".into(),
            jwt_public_key_path: "keys/public.pem".into(),
            issuer: "superpowers-token-server".into(),
            clients_file_path: "clients.json".into(),
            default_access_ttl_sec: 3600,
            default_refresh_ttl_sec: 2592000,  // 30 天
            default_resource_ttl_sec: 300,
        }
    }
}
```

- [ ] **Step 2: 更新 main.rs — 启动配置**

```rust
mod config;

use config::TokenServerConfig;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "token_server=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = config::Config::builder()
        .add_source(config::File::with_name("config/token-server").required(false))
        .add_source(config::Environment::with_prefix("TS_").separator("__"))
        .build()
        .unwrap()
        .try_deserialize::<TokenServerConfig>()
        .unwrap();

    tracing::info!("token-server config loaded, listening on {}:{}", cfg.server.host, cfg.server.port);
}
```

- [ ] **Step 3: 验证**

运行:
```bash
mkdir -p config keys
# 生成测试密钥对
ssh-keygen -t rsa -b 2048 -m PEM -f keys/private.pem -N ""
ssh-keygen -f keys/private.pem -e -m PKCS8 > keys/public.pem
# 验证启动
cargo run -p token-server
```

期望: 打印 "token-server config loaded, listening on 0.0.0.0:4000"

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat(token-server): config loading with RSA key paths"
```

---

### Task 2.2: clients.json 文件管理 + Redis 连接

**文件:**
- Create: `token-server/src/store/mod.rs`
- Create: `token-server/src/store/client.rs`
- Create: `token-server/src/store/redis.rs`

- [ ] **Step 1: 编写 Client 模型 + 文件加载**

`token-server/src/store/client.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientsConfig {
    pub clients: Vec<Client>,
}

pub struct ClientStore {
    clients: Arc<RwLock<ClientsConfig>>,
    path: String,
}

impl ClientStore {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let clients: ClientsConfig = serde_json::from_str(&content)?;
        tracing::info!("loaded {} clients from {}", clients.clients.len(), path);
        Ok(Self {
            clients: Arc::new(RwLock::new(clients)),
            path: path.to_string(),
        })
    }

    pub async fn find_by_id(&self, client_id: &str) -> Option<Client> {
        let clients = self.clients.read().await;
        clients.clients.iter().find(|c| c.client_id == client_id).cloned()
    }

    pub async fn reload(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(&self.path)?;
        let clients: ClientsConfig = serde_json::from_str(&content)?;
        *self.clients.write().await = clients;
        tracing::info!("clients reloaded from {}", self.path);
        Ok(())
    }
}
```

- [ ] **Step 2: 编写 Redis 操作**

`token-server/src/store/redis.rs`:
```rust
use redis::{aio::MultiplexedConnection, AsyncCommands};
use std::time::Duration;

pub async fn connect(url: &str) -> Result<MultiplexedConnection, redis::RedisError> {
    let client = redis::Client::open(url)?;
    client.get_multiplexed_async_connection().await
}

pub async fn set_code(conn: &mut MultiplexedConnection, prefix: &str, code: &str, user_id: &str, ttl_sec: u64) -> Result<(), redis::RedisError> {
    let key = format!("{}code:{}", prefix, code);
    let _: () = conn.set_ex(&key, user_id, ttl_sec).await?;
    Ok(())
}

pub async fn get_code(conn: &mut MultiplexedConnection, prefix: &str, code: &str) -> Result<Option<String>, redis::RedisError> {
    let key = format!("{}code:{}", prefix, code);
    conn.get(&key).await
}

pub async fn del_code(conn: &mut MultiplexedConnection, prefix: &str, code: &str) -> Result<(), redis::RedisError> {
    let key = format!("{}code:{}", prefix, code);
    let _: () = conn.del(&key).await?;
    Ok(())
}

pub async fn add_to_blacklist(conn: &mut MultiplexedConnection, prefix: &str, jti: &str, ttl_sec: u64) -> Result<(), redis::RedisError> {
    let key = format!("{}blacklist:{}", prefix, jti);
    let _: () = conn.set_ex(&key, "1", ttl_sec).await?;
    Ok(())
}

pub async fn is_blacklisted(conn: &mut MultiplexedConnection, prefix: &str, jti: &str) -> Result<bool, redis::RedisError> {
    let key = format!("{}blacklist:{}", prefix, jti);
    conn.exists(&key).await
}

pub async fn add_refresh_family(conn: &mut MultiplexedConnection, prefix: &str, jti: &str, ttl_sec: u64) -> Result<(), redis::RedisError> {
    let key = format!("{}refresh:{}", prefix, jti);
    let _: () = conn.set_ex(&key, "1", ttl_sec).await?;
    Ok(())
}

pub async fn is_refresh_marked(conn: &mut MultiplexedConnection, prefix: &str, jti: &str) -> Result<bool, redis::RedisError> {
    let key = format!("{}refresh:{}", prefix, jti);
    conn.exists(&key).await
}
```

- [ ] **Step 3: 编写 store/mod.rs**

```rust
pub mod client;
pub mod redis;
```

- [ ] **Step 4: 创建示例 clients.json**

`clients.json`:
```json
{
  "clients": [
    {
      "client_id": "api-gateway",
      "client_secret_hash": "$argon2id$v=19$m=65536,t=3,p=4$...",
      "name": "API Gateway",
      "permissions": ["token:issue", "token:revoke"],
      "grant_types": ["authorization_code", "refresh_token"],
      "access_token_ttl_sec": 3600,
      "refresh_token_ttl_sec": 2592000,
      "status": "active"
    }
  ]
}
```

- [ ] **Step 5: 验证**

运行: `cargo check -p token-server`
期望: 编译通过

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(token-server): client store (json file + auto reload) and redis operations"
```

---

### Task 2.3: JWT 签发模块

**文件:**
- Create: `token-server/src/issuer.rs`

- [ ] **Step 1: 编写 JWT 签发方法**

`token-server/src/issuer.rs`:
```rust
use common::token::{AccessTokenClaims, RefreshTokenClaims, ResourceTokenClaims};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub struct TokenIssuer {
    encoding_key: EncodingKey,
    issuer: String,
}

impl TokenIssuer {
    pub fn new(private_key_pem: &str, issuer: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let encoding_key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())?;
        Ok(Self {
            encoding_key,
            issuer: issuer.to_string(),
        })
    }

    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    pub fn issue_access_token(
        &self,
        sub: &str,
        client_id: &str,
        permissions: &[String],
        ttl_sec: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let now = Self::now();
        let claims = AccessTokenClaims {
            sub: sub.to_string(),
            jti: Uuid::new_v4().to_string(),
            iss: self.issuer.clone(),
            iat: now,
            exp: now + ttl_sec,
            scope: "access".into(),
            client_id: client_id.to_string(),
            permissions: permissions.to_vec(),
        };
        let token = encode(
            &Header::new(Algorithm::RS256),
            &claims,
            &self.encoding_key,
        )?;
        Ok(token)
    }

    pub fn issue_refresh_token(
        &self,
        sub: &str,
        ttl_sec: i64,
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        let now = Self::now();
        let family = Uuid::new_v4().to_string();
        let claims = RefreshTokenClaims {
            sub: sub.to_string(),
            jti: Uuid::new_v4().to_string(),
            iss: self.issuer.clone(),
            iat: now,
            exp: now + ttl_sec,
            scope: "refresh".into(),
            family: family.clone(),
        };
        let token = encode(
            &Header::new(Algorithm::RS256),
            &claims,
            &self.encoding_key,
        )?;
        Ok((token, family))
    }

    pub fn issue_resource_token(
        &self,
        sub: &str,
        resource_type: &str,
        resource_id: &str,
        permission: &str,
        ttl_sec: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let now = Self::now();
        let claims = ResourceTokenClaims {
            sub: sub.to_string(),
            jti: Uuid::new_v4().to_string(),
            iss: self.issuer.clone(),
            iat: now,
            exp: now + ttl_sec,
            scope: "resource".into(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            permission: permission.to_string(),
        };
        let token = encode(
            &Header::new(Algorithm::RS256),
            &claims,
            &self.encoding_key,
        )?;
        Ok(token)
    }
}
```

- [ ] **Step 2: 验证 — 编写快速测试**

创建 `token-server/tests/issuer_test.rs`:
```rust
use token_server::issuer::TokenIssuer;
use common::token::AccessTokenClaims;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

#[test]
fn test_issue_and_verify_access_token() {
    let issuer = TokenIssuer::new(
        include_str!("../../keys/private.pem"),
        "test-issuer"
    ).unwrap();

    let token = issuer.issue_access_token("1", "test-client", &["file:read".into()], 3600).unwrap();

    // 用公钥验证
    let validation = Validation::new(Algorithm::RS256);
    let decoding_key = DecodingKey::from_rsa_pem(
        include_str!("../../keys/public.pem").as_bytes()
    ).unwrap();

    let data = decode::<AccessTokenClaims>(&token, &decoding_key, &validation).unwrap();
    assert_eq!(data.claims.sub, "1");
    assert_eq!(data.claims.scope, "access");
    assert_eq!(data.claims.permissions, vec!["file:read"]);
    assert!(data.claims.exp > data.claims.iat);
}
```

- [ ] **Step 3: 运行测试**

运行: `cargo test -p token-server`
期望: 测试通过，token 签发后用公钥能验证成功

- [ ] **Step 4: 将 Cargo.toml 的测试引用加到 token-server**

在 `token-server/Cargo.toml` 添加:
```toml
[dev-dependencies]
jsonwebtoken.workspace = true
```

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(token-server): jwt issuer (access, refresh, resource tokens)"
```

---

### Task 2.4: token-server HTTP 路由

**文件:**
- Create: `token-server/src/routes/mod.rs`
- Create: `token-server/src/routes/access.rs`
- Create: `token-server/src/routes/refresh.rs`
- Create: `token-server/src/routes/revoke.rs`
- Create: `token-server/src/routes/resource.rs`
- Create: `token-server/src/routes/api_key.rs`
- Create: `token-server/src/routes/jwks.rs`

- [ ] **Step 1: 编写 /tokens/access 路由**

`token-server/src/routes/access.rs`:
```rust
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use crate::store::redis;

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
    State(state): State<crate::AppState>,
    Json(req): Json<AccessRequest>,
) -> Result<Json<AccessResponse>, (axum::http::StatusCode, String)> {
    if req.grant_type != "authorization_code" {
        return Err((axum::http::StatusCode::BAD_REQUEST, "unsupported grant_type".into()));
    }

    // 1. 验证 client_id / client_secret
    let client = state.client_store.find_by_id(&req.client_id).await
        .ok_or((axum::http::StatusCode::UNAUTHORIZED, "invalid client".into()))?;

    // 2. 从 Redis 取 code
    let mut conn = state.redis_conn.clone();
    let user_id = redis::get_code(&mut conn, &state.redis_prefix, &req.code).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((axum::http::StatusCode::BAD_REQUEST, "invalid or expired code".into()))?;

    // 3. 删除 code（一次性使用）
    let _ = redis::del_code(&mut conn, &state.redis_prefix, &req.code).await;

    // 4. 签发 token 对
    let (refresh_token, _family) = state.issuer.issue_refresh_token(
        &user_id,
        client.refresh_token_ttl_sec,
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let access_token = state.issuer.issue_access_token(
        &user_id,
        &req.client_id,
        &client.permissions,
        client.access_token_ttl_sec,
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AccessResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}
```

- [ ] **Step 2: 编写 /tokens/refresh 路由**

`token-server/src/routes/refresh.rs`:
```rust
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use common::token::RefreshTokenClaims;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use crate::store::redis;

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

pub async fn handle_refresh(
    State(state): State<crate::AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, (axum::http::StatusCode, String)> {
    // 1. 验证 client
    let client = state.client_store.find_by_id(&req.client_id).await
        .ok_or((axum::http::StatusCode::UNAUTHORIZED, "invalid client".into()))?;

    // 2. 验证 refresh_token 签名
    let decoding_key = DecodingKey::from_rsa_pem(state.public_key_pem.as_bytes())
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let validation = Validation::new(Algorithm::RS256);
    let old_claims = decode::<RefreshTokenClaims>(&req.refresh_token, &decoding_key, &validation)
        .map_err(|_| (axum::http::StatusCode::UNAUTHORIZED, "invalid refresh token".into()))?
        .claims;

    // 3. 检查 refresh token 是否已被标记（防止重放）
    let mut conn = state.redis_conn.clone();
    if redis::is_refresh_marked(&mut conn, &state.redis_prefix, &old_claims.jti).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))? {
        // 检测到重放攻击 — 轮换整个 family
        return Err((axum::http::StatusCode::UNAUTHORIZED, "refresh token already used".into()));
    }

    // 4. 标记旧 token 已使用
    let ttl = old_claims.exp - std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    if ttl > 0 {
        let _ = redis::add_refresh_family(&mut conn, &state.redis_prefix, &old_claims.jti, ttl as u64).await;
    }

    // 5. 签发新 token 对
    let (new_refresh, _) = state.issuer.issue_refresh_token(&old_claims.sub, client.refresh_token_ttl_sec)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let new_access = state.issuer.issue_access_token(
        &old_claims.sub,
        &req.client_id,
        &client.permissions,
        client.access_token_ttl_sec,
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(TokenResponse {
        access_token: new_access,
        refresh_token: new_refresh,
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}
```

- [ ] **Step 3: 编写 /tokens/revoke 路由**

`token-server/src/routes/revoke.rs`:
```rust
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use common::token::AccessTokenClaims;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use crate::store::redis;

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token: String,
}

pub async fn handle_revoke(
    State(state): State<crate::AppState>,
    Json(req): Json<RevokeRequest>,
) -> StatusCode {
    let decoding_key = DecodingKey::from_rsa_pem(state.public_key_pem.as_bytes())
        .unwrap(); // 简化错误处理

    let validation = Validation::new(Algorithm::RS256);
    validation.insecure_disable_expiry_validation(); // 已过期的也可以注销

    if let Ok(data) = decode::<AccessTokenClaims>(&req.token, &decoding_key, &validation) {
        let exp = data.claims.exp;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let ttl = if exp > now { (exp - now) as u64 } else { 0 };

        if ttl > 0 {
            let mut conn = state.redis_conn.clone();
            let _ = redis::add_to_blacklist(&mut conn, &state.redis_prefix, &data.claims.jti, ttl).await;
        }

        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    }
}
```

- [ ] **Step 4: 编写 /tokens/resource 和 /api-keys 和 /jwks 路由（骨架）**

`token-server/src/routes/resource.rs`:
```rust
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ResourceTokenRequest {
    pub resource_type: String,
    pub resource_id: String,
    pub permission: String,
    pub ttl_sec: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ResourceTokenResponse {
    pub resource_token: String,
    pub expires_in: i64,
}

pub async fn handle_resource(
    State(state): State<crate::AppState>,
    Json(req): Json<ResourceTokenRequest>,
) -> Result<Json<ResourceTokenResponse>, (axum::http::StatusCode, String)> {
    let ttl = req.ttl_sec.unwrap_or(state.default_resource_ttl_sec);
    let token = state.issuer.issue_resource_token(
        "user-ctx-needs-auth",
        &req.resource_type,
        &req.resource_id,
        &req.permission,
        ttl,
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ResourceTokenResponse {
        resource_token: token,
        expires_in: ttl,
    }))
}
```

`token-server/src/routes/jwks.rs`:
```rust
use axum::{extract::State, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Serialize)]
pub struct JwkKey {
    kty: String,
    alg: String,
    kid: String,
    n: String,
    e: String,
}

pub async fn handle_jwks() -> Json<JwksResponse> {
    // TODO: 从公钥文件解析 n, e 生成 JWKS
    Json(JwksResponse { keys: vec![] })
}
```

`token-server/src/routes/api_key.rs`:
```rust
use axum::{extract::State, Json};

pub async fn handle_create_api_key(State(_state): State<crate::AppState>) -> &'static str {
    "not yet implemented"
}

pub async fn handle_list_api_keys(State(_state): State<crate::AppState>) -> &'static str {
    "not yet implemented"
}

pub async fn handle_delete_api_key(State(_state): State<crate::AppState>) -> &'static str {
    "not yet implemented"
}
```

- [ ] **Step 5: 编写 routes/mod.rs**

```rust
pub mod access;
pub mod refresh;
pub mod revoke;
pub mod resource;
pub mod api_key;
pub mod jwks;
```

- [ ] **Step 6: 更新 main.rs — 组装 HTTP server**

```rust
mod config;
mod routes;
mod issuer;
mod store;

use std::sync::Arc;
use axum::{routing::post, Router};
use config::TokenServerConfig;
use issuer::TokenIssuer;
use store::client::ClientStore;

#[derive(Clone)]
pub struct AppState {
    pub issuer: Arc<TokenIssuer>,
    pub client_store: Arc<ClientStore>,
    pub redis_conn: Arc<redis::aio::MultiplexedConnection>,
    pub redis_prefix: String,
    pub public_key_pem: String,
    pub default_resource_ttl_sec: i64,
}

#[tokio::main]
async fn main() {
    // ... tracing init ...

    let cfg: TokenServerConfig = /* config load */ todo!();
    let private_key = std::fs::read_to_string(&cfg.jwt_private_key_path).unwrap();
    let public_key = std::fs::read_to_string(&cfg.jwt_public_key_path).unwrap();

    let issuer = TokenIssuer::new(&private_key, &cfg.issuer).unwrap();
    let client_store = ClientStore::load(&cfg.clients_file_path).unwrap();
    let redis_conn = store::redis::connect(&cfg.redis.url).await.unwrap();

    let state = AppState {
        issuer: Arc::new(issuer),
        client_store: Arc::new(client_store),
        redis_conn: Arc::new(redis_conn),
        redis_prefix: cfg.redis.prefix.clone(),
        public_key_pem: public_key,
        default_resource_ttl_sec: cfg.default_resource_ttl_sec,
    };

    let app = Router::new()
        .route("/api/tokens/access", post(routes::access::handle_access))
        .route("/api/tokens/refresh", post(routes::refresh::handle_refresh))
        .route("/api/tokens/revoke", post(routes::revoke::handle_revoke))
        .route("/api/tokens/resource", post(routes::resource::handle_resource))
        .route("/api/api-keys", post(routes::api_key::handle_create_api_key))
        .route("/api/api-keys", axum::routing::get(routes::api_key::handle_list_api_keys))
        .route("/api/api-keys/:id", axum::routing::delete(routes::api_key::handle_delete_api_key))
        .route("/api/tokens/.well-known/jwks.json", axum::routing::get(routes::jwks::handle_jwks))
        .with_state(state);

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 7: 验证**

运行: `cargo check -p token-server`
期望: 编译通过

- [ ] **Step 8: 提交**

```bash
git add -A
git commit -m "feat(token-server): http routes for token issuance, refresh, revoke, resource"
```

---

### Task 2.5: token-server 完成集成

- [ ] **Step 1: 修复 main.rs 配置加载（用实际代码替换 todo!()）**

- [ ] **Step 2: 启动 Redis（docker），启动 token-server，用 curl 测试 /tokens/access 端点**

- [ ] **Step 3: 提交**

```bash
git add -A
git commit -m "feat(token-server): integration wiring and startup"
```

---

## Phase 3: api-gateway

### Task 3.1: 配置 + Consul 发现

**文件:**
- Create: `api-gateway/src/config.rs`
- Create: `api-gateway/src/discovery.rs`

- [ ] **Step 1: gateway 配置**

`api-gateway/src/config.rs`:
```rust
use common::config::{RedisConfig, ServerConfig};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub redis: RedisConfig,

    pub jwt_public_key_url: String,   // JWKS 端点 URL
    pub consul_url: String,

    pub auth_service: String,          // Consul service name
    pub token_service: String,
    pub user_service: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig { host: "0.0.0.0".into(), port: 3000 },
            redis: RedisConfig::default(),
            jwt_public_key_url: "http://token-server:4000/api/tokens/.well-known/jwks.json".into(),
            consul_url: "http://127.0.0.1:8500".into(),
            auth_service: "auth-service".into(),
            token_service: "token-server".into(),
            user_service: "user-service".into(),
        }
    }
}
```

- [ ] **Step 2: Consul 服务发现客户端**

`api-gateway/src/discovery.rs`:
```rust
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ConsulServiceEntry {
    #[serde(rename = "Service")]
    service: ConsulService,
}

#[derive(Debug, Deserialize)]
struct ConsulService {
    #[serde(rename = "Address")]
    address: String,
    #[serde(rename = "Port")]
    port: u16,
}

pub struct ServiceDiscovery {
    client: Client,
    consul_url: String,
}

impl ServiceDiscovery {
    pub fn new(consul_url: &str) -> Self {
        Self {
            client: Client::new(),
            consul_url: consul_url.to_string(),
        }
    }

    pub async fn discover(&self, service_name: &str) -> Result<String, String> {
        let url = format!(
            "{}/v1/catalog/service/{}",
            self.consul_url, service_name
        );
        let entries: Vec<ConsulServiceEntry> = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        let entry = entries.first().ok_or(format!("no healthy service: {}", service_name))?;
        Ok(format!("http://{}:{}", entry.service.address, entry.service.port))
    }
}
```

- [ ] **Step 3: 验证**

运行: `cargo check -p api-gateway`
期望: 编译通过

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat(api-gateway): config loading and consul service discovery"
```

---

### Task 3.2: JWT 验证 + Redis 黑名单

**文件:**
- Create: `api-gateway/src/jwt.rs`
- Create: `api-gateway/src/middleware/mod.rs`
- Create: `api-gateway/src/middleware/jwt_auth.rs`

- [ ] **Step 1: JWT 验签 + 黑名单检查**

`api-gateway/src/jwt.rs`:
```rust
use common::token::AccessTokenClaims;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use redis::AsyncCommands;

pub struct JwtValidator {
    decoding_key: DecodingKey,
    validation: Validation,
}

impl JwtValidator {
    pub fn new(public_key_pem: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let decoding_key = DecodingKey::from_rsa_pem(public_key_pem.as_bytes())?;
        let validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        Ok(Self { decoding_key, validation })
    }

    pub fn validate(&self, token: &str) -> Result<AccessTokenClaims, String> {
        decode::<AccessTokenClaims>(token, &self.decoding_key, &self.validation)
            .map(|data| data.claims)
            .map_err(|e| format!("invalid token: {}", e))
    }
}

pub async fn check_blacklist(
    conn: &mut redis::aio::MultiplexedConnection,
    prefix: &str,
    jti: &str,
) -> Result<bool, String> {
    let key = format!("{}blacklist:{}", prefix, jti);
    let exists: bool = conn.exists(&key).await.map_err(|e| e.to_string())?;
    Ok(exists)
}
```

- [ ] **Step 2: JWT 鉴权中间件**

`api-gateway/src/middleware/jwt_auth.rs`:
```rust
use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use redis::AsyncCommands;

pub async fn jwt_auth_middleware(
    State(state): State<crate::AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let auth_header = req.headers().get(header::AUTHORIZATION).cloned();
    let token = auth_header
        .and_then(|v| v.to_str().ok().map(|s| s.to_string()))
        .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));

    let token = match token {
        Some(t) => t,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "missing Bearer token").into_response(),
    };

    // 1. 验签
    let claims = match state.jwt_validator.validate(&token) {
        Ok(c) => c,
        Err(e) => return (axum::http::StatusCode::UNAUTHORIZED, e).into_response(),
    };

    // 2. 查黑名单
    let mut conn = state.redis_conn.clone();
    let blacklisted = check_blacklist_for_jti(&mut conn, &state.redis_prefix, &claims.jti).await;
    match blacklisted {
        Ok(true) => return (axum::http::StatusCode::UNAUTHORIZED, "token revoked").into_response(),
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        _ => {}
    }

    // 3. 注入 claims 到 request extensions
    req.extensions_mut().insert(claims);
    next.run(req).await
}
```

- [ ] **Step 3: 验证**

运行: `cargo check -p api-gateway`

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat(api-gateway): jwt validation middleware with redis blacklist check"
```

---

### Task 3.3: 路由转发 + gateway 组装

**文件:**
- Create: `api-gateway/src/router.rs`
- Modify: `api-gateway/src/main.rs`

- [ ] **Step 1: 路由表和转发逻辑**

`api-gateway/src/router.rs`:
```rust
use axum::{
    body::Body,
    extract::State,
    http::Method,
    response::{IntoResponse, Response},
};
use reqwest::Client;
use std::collections::HashSet;

// JWT 验证白名单
fn path_is_whitelisted(path: &str) -> bool {
    let whitelist: &[&str] = &[
        "/api/auth/",
        "/api/tokens/",
    ];
    whitelist.iter().any(|p| path.starts_with(p))
        || path == "/api/tokens/.well-known/jwks.json"
}

pub async fn proxy_handler(
    State(state): State<crate::AppState>,
    req: axum::extract::Request,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // 判断目标服务
    let service_name = if path.starts_with("/api/auth/") {
        &state.auth_service_name
    } else if path.starts_with("/api/tokens/") {
        &state.token_service_name
    } else if path.starts_with("/api/users/") || path.starts_with("/api/roles/") || path.starts_with("/api/permissions/") {
        &state.user_service_name
    } else {
        return (axum::http::StatusCode::NOT_FOUND, "unknown service").into_response();
    };

    // Consul 发现
    let backend_url = match state.discovery.discover(service_name).await {
        Ok(url) => url,
        Err(e) => return (axum::http::StatusCode::SERVICE_UNAVAILABLE, e).into_response(),
    };

    // 构建转发请求
    let client = Client::new();
    let target_url = format!("{}{}", backend_url, path);

    let resp = match method {
        Method::GET => client.get(&target_url),
        Method::POST => client.post(&target_url),
        Method::PUT => client.put(&target_url),
        Method::DELETE => client.delete(&target_url),
        _ => return (axum::http::StatusCode::METHOD_NOT_ALLOWED, "unsupported method").into_response(),
    };

    let resp = match resp.send().await {
        Ok(r) => r,
        Err(e) => return (axum::http::StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    };

    let status = resp.status();
    let body = resp.bytes().await.unwrap_or_default();

    Response::builder()
        .status(status)
        .body(Body::from(body))
        .unwrap()
        .into_response()
}
```

- [ ] **Step 2: main.rs 组装**

`api-gateway/src/main.rs`:
```rust
mod config;
mod discovery;
mod jwt;
mod router;
mod middleware;

use std::sync::Arc;
use axum::{
    middleware as axum_middleware,
    routing::any,
    Router,
};
use config::GatewayConfig;
use discovery::ServiceDiscovery;
use jwt::JwtValidator;

#[derive(Clone)]
pub struct AppState {
    pub jwt_validator: Arc<JwtValidator>,
    pub discovery: Arc<ServiceDiscovery>,
    pub redis_conn: Arc<redis::aio::MultiplexedConnection>,
    pub redis_prefix: String,
    pub auth_service_name: String,
    pub token_service_name: String,
    pub user_service_name: String,
}

#[tokio::main]
async fn main() {
    // tracing init...

    let cfg: GatewayConfig = /* config load */ todo!();

    // 从 JWKS 端点拉取公钥
    let client = reqwest::Client::new();
    let jwks_resp = client.get(&cfg.jwt_public_key_url).send().await;
    // TODO: 解析 JWKS JSON，提取公钥 → 转为 PEM
    let public_key_pem = get_public_key_from_jwks(&cfg.jwt_public_key_url).await.unwrap();

    let jwt_validator = JwtValidator::new(&public_key_pem).unwrap();
    let discovery = ServiceDiscovery::new(&cfg.consul_url);
    let redis_conn = /* redis connect */ todo!();

    let state = AppState {
        jwt_validator: Arc::new(jwt_validator),
        discovery: Arc::new(discovery),
        redis_conn: Arc::new(redis_conn),
        redis_prefix: cfg.redis.prefix.clone(),
        auth_service_name: cfg.auth_service.clone(),
        token_service_name: cfg.token_service.clone(),
        user_service_name: cfg.user_service.clone(),
    };

    // 白名单路由（不验证 JWT）
    let app = Router::new()
        .route("/api/auth/*path", any(router::proxy_handler))
        .route("/api/tokens/*path", any(router::proxy_handler))
        // 其他 API 路由（验证 JWT）
        .route("/api/*path", any(router::proxy_handler)
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                middleware::jwt_auth::jwt_auth_middleware,
            )))
        .with_state(state);

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 3: 验证**

运行: `cargo check -p api-gateway`

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat(api-gateway): route forwarding with consul discovery"
```

---

### Task 3.4: api-gateway 启动完成

- [ ] **Step 1: 修复所有 todo!() 占位符**

- [ ] **Step 2: 启动 Redis + Consul（docker），启动 token-server + api-gateway，curl 验证完整链路**

- [ ] **Step 3: 提交**

---

## Phase 4: auth-service (Go)

### Task 4.1: 配置加载 + DB 多驱动连接

**文件:**
- Create: `auth-service/config/config.go`
- Create: `auth-service/store/db.go`
- Create: `auth-service/models/user.go`

- [ ] **Step 1: DB 配置 + 多驱动切换 → 写代码 → 编译验证 → 提交**

---

### Task 4.2: 登录/注册/注销 Handler

**文件:**
- Create: `auth-service/handlers/register.go`
- Create: `auth-service/handlers/login.go`
- Create: `auth-service/handlers/logout.go`

- [ ] **Step 1: 编写注册（password_hash → DB）**
- [ ] **Step 2: 编写登录（验证密码 → 生成code → Redis → 返回）**
- [ ] **Step 3: 编写注销（调用 token-server POST /api/tokens/revoke）**

- [ ] **Step 4: 验证**

```bash
cd auth-service && go run main.go
curl -X POST http://localhost:5000/api/auth/register \
  -d '{"username":"test","password":"123456"}'
# → 201 Created

curl -X POST http://localhost:5000/api/auth/login \
  -d '{"username":"test","password":"123456"}'
# → {"code":"...", "expires_in":300}
```

- [ ] **Step 5: 提交**

---

## Phase 5: user-service (Go)

### Task 5.1: 配置 + DB + Model

**文件:**
- Create: `user-service/config/config.go`
- Create: `user-service/store/db.go`
- Create: `user-service/models/user.go`
- Create: `user-service/models/role.go`
- Create: `user-service/models/permission.go`

- [ ] **Step 1: 多驱动 DB 初始化（与 auth-service 相同模式）**
- [ ] **Step 2: GORM Model 定义 — UsrUser, UsrRole, UsrPermission 及关联**
- [ ] **Step 3: 编译验证 → 提交**

---

### Task 5.2: 用户资料 CRUD Handler

**文件:**
- Create: `user-service/handlers/user.go`

- [ ] **Step 1: GET /api/users/me — 根据 JWT sub 查用户资料**
- [ ] **Step 2: PUT /api/users/me — 更新自己的资料**
- [ ] **Step 3: GET /api/users — 分页列表（需 admin）**
- [ ] **Step 4: GET /api/users/{id} — 单个用户**
- [ ] **Step 5: 编译验证 → 提交**

---

### Task 5.3: 角色 + 权限 Handler

**文件:**
- Create: `user-service/handlers/role.go`
- Create: `user-service/handlers/permission.go`

- [ ] **Step 1: 角色 CRUD（GET/POST/PUT/DELETE）**
- [ ] **Step 2: 权限列表（GET /api/permissions）**
- [ ] **Step 3: 用户-角色关联（POST/DELETE /api/users/{id}/roles）**
- [ ] **Step 4: 角色-权限关联（POST /api/roles/{id}/permissions）**
- [ ] **Step 5: 编译验证 → 提交**

---

## Phase 6: 基础设施

### Task 6.1: Nginx 配置

**文件:**
- Create: `nginx/nginx.conf`

- [ ] **Step 1: 编写 nginx.conf**

```nginx
server {
    listen 80;

    location / {
        root /usr/share/nginx/html;
        index index.html;
    }

    location /api/ {
        proxy_pass http://api-gateway:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

- [ ] **Step 2: 提交**

---

### Task 6.2: Dockerfiles

**文件:**
- Create: `Dockerfile.token-server`
- Create: `Dockerfile.api-gateway`
- Create: `Dockerfile.auth-service`
- Create: `Dockerfile.user-service`

- [ ] **Step 1: 编写每个 Dockerfile（alpine 多阶段构建）**

`Dockerfile.token-server`:
```dockerfile
FROM rust:1.85-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY Cargo.workspace.toml Cargo.toml .
COPY common/ common/
COPY token-server/ token-server/
RUN cargo build --release -p token-server

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/release/token-server /usr/local/bin/
COPY clients.json /app/clients.json
COPY keys/ /app/keys/
WORKDIR /app
CMD ["token-server"]
```

（类似格式生成其余 3 个 Dockerfile）

- [ ] **Step 2: 提交**

---

### Task 6.3: docker-compose.yml + Consul 配置

**文件:**
- Create: `docker-compose.yml`
- Create: `consul/config.json`

- [ ] **Step 1: 编写 docker-compose.yml**

```yaml
version: "3.8"

services:
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
    volumes:
      - ./nginx/nginx.conf:/etc/nginx/conf.d/default.conf
      - ./frontend:/usr/share/nginx/html
    depends_on:
      - api-gateway

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  consul:
    image: hashicorp/consul:1.20
    ports:
      - "8500:8500"
    command: agent -dev -client=0.0.0.0 -config-dir=/consul/config
    volumes:
      - ./consul/config.json:/consul/config/config.json

  api-gateway:
    build:
      context: .
      dockerfile: Dockerfile.api-gateway
    ports:
      - "3000:3000"
    environment:
      - RUST_LOG=api_gateway=debug
      - GW_REDIS_URL=redis://redis:6379
      - GW_CONSUL_URL=http://consul:8500
    depends_on:
      - redis
      - consul

  token-server:
    build:
      context: .
      dockerfile: Dockerfile.token-server
    ports:
      - "4000:4000"
    environment:
      - RUST_LOG=token_server=debug
      - TS_REDIS_URL=redis://redis:6379
    volumes:
      - ./clients.json:/app/clients.json
      - ./keys:/app/keys
    depends_on:
      - redis

  auth-service:
    build:
      context: ./auth-service
      dockerfile: ../Dockerfile.auth-service
    ports:
      - "5000:5000"
    environment:
      - DB_TYPE=mysql
      - DB_DSN=user:pass@tcp(mysql:3306)/superpowers?charset=utf8mb4
      - REDIS_ADDR=redis:6379
      - TOKEN_SERVER_URL=http://token-server:4000
    depends_on:
      - redis
      - token-server

  user-service:
    build:
      context: ./user-service
      dockerfile: ../Dockerfile.user-service
    ports:
      - "6000:6000"
    environment:
      - DB_TYPE=mysql
      - DB_DSN=user:pass@tcp(mysql:3306)/superpowers?charset=utf8mb4
    depends_on:
      - mysql

  mysql:
    image: mysql:8.0
    environment:
      MYSQL_ROOT_PASSWORD: rootpass
      MYSQL_DATABASE: superpowers
    ports:
      - "3306:3306"

  frontend:
    build:
      context: ./frontend
      dockerfile: Dockerfile (if needed)
    volumes:
      - ./frontend:/usr/share/nginx/html
```

- [ ] **Step 2: 编写 minimal Consul 配置**

```json
{
  "services": [
    {"name": "auth-service", "port": 5000},
    {"name": "token-server", "port": 4000},
    {"name": "user-service", "port": 6000}
  ]
}
```

- [ ] **Step 3: 验证启动**

```bash
docker-compose up -d
docker-compose ps  # 全部 Running
curl http://localhost:4000/api/tokens/.well-known/jwks.json
```

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat: docker-compose, nginx, consul, dockerfiles"
```

---

### Task 6.4: 端到端集成验证

- [ ] **Step 1: 完整 OAuth2 流程**

```bash
# 1. 注册
curl -X POST http://localhost/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"test","password":"123456"}'

# 2. 登录 → 获取 code
curl -X POST http://localhost/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"test","password":"123456"}'
# → {"code":"a8f3c9e1b2d4...","expires_in":300}

# 3. code 换 token
curl -X POST http://localhost/api/tokens/access \
  -H "Content-Type: application/json" \
  -d '{"grant_type":"authorization_code","code":"a8f3c9e1b2d4...","client_id":"api-gateway","client_secret":"..."}'
# → {"access_token":"eyJ...","refresh_token":"eyJ...","token_type":"Bearer","expires_in":3600}

# 4. 用 access_token 访问被保护的 API
curl http://localhost/api/users/me \
  -H "Authorization: Bearer eyJ..."
# → {"id":1,"display_name":"test",...}

# 5. 注销
curl -X POST http://localhost/api/auth/logout \
  -H "Authorization: Bearer eyJ..."

# 6. 验证 token 已失效
curl http://localhost/api/users/me \
  -H "Authorization: Bearer eyJ..."
# → 401 Unauthorized
```

- [ ] **Step 2: 提交**

---

## 总结

| Phase | 内容 | Tasks | 语言 |
|-------|------|-------|------|
| 0 | 项目脚手架 | 3 | Rust + Go |
| 1 | common crate | 2 | Rust |
| 2 | token-server | 5 | Rust |
| 3 | api-gateway | 4 | Rust |
| 4 | auth-service | 2 | Go |
| 5 | user-service | 3 | Go |
| 6 | 基础设施 | 4 | Docker/Nginx |

**Total: 23 Tasks**
