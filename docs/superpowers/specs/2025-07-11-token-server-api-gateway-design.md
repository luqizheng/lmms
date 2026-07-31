# Token Server + API Gateway 系统设计文档

> 设计日期: 2025-07-11
> 状态: Draft → 待审核

---

## 1. 概述

构建一套完整的 API 鉴权基础设施，包含：

- **api-gateway** (Rust) — 统一 API 入口，JWT 离线验签 + Redis 黑名单 + Consul 服务发现 + 路由转发
- **token-server** (Rust) — OAuth2 Authorization Code 签发 JWT，API Key 管理，Resource Token
- **auth-service** (Go) — 用户密码认证，签发 authorization code，后续扩展多登录方式
- **user-service** (Go) — 用户资料 CRUD，RBAC 权限/角色管理

所有服务通过 **Nginx** 统一反代，**Consul** 做服务注册发现，**Docker Compose** 编排部署。

---

## 2. 系统架构

```
                     Nginx (:80)
                   /              \
             /                   /api/*
         Frontend              api-gateway (Rust · axum)
         (静态文件)               │
                                 │ JWT 离线验签 (RSA公钥)
                                 │ Redis 黑名单查询
                                 │ Consul 服务发现
                                 │ 请求路由转发
                                 │
                   ┌─────────────┼─────────────┐
                   │             │             │
           token-server    auth-service   user-service
           (Rust · axum)   (Go · gin)     (Go · gin)
           ────────────    ───────────    ───────────
           签发 JWT         用户密码登录     用户资料 CRUD
           API Key 管理     生成 auth code  RBAC 权限管理
           Refresh 换发     支持扩展多登录   角色-权限关联
           Resource Token
           ────────────    ───────────    ───────────
           client→JSON文件   DB: MySQL/      DB: MySQL/
           黑名单→Redis      MSSQL/DM        MSSQL/DM
           code→Redis       (表前缀 auth_)  (表前缀 usr_)
```

### 流量路径

| 路径 | 处理方式 |
|------|---------|
| `/` | Nginx 直接 serve 前端静态文件 |
| `/api/auth/*` | api-gateway **不验证 JWT**，透传到 auth-service |
| `/api/tokens/*` | api-gateway **不验证 JWT**，透传到 token-server（换 token 用） |
| `/api/*` | api-gateway **验证 JWT + 黑名单**，通过后 Consul 发现转发 |
| `/health` | 各服务提供健康检查端点 |

---

## 3. OAuth2 Authorization Code 流程

```
 客户端              auth-service(Go)      token-server(Rust)     gateway(Rust)
   │                      │                      │                   │
   │ POST /api/auth/login │                      │                   │
   │ {username, password} │                      │                   │
   │─────────────────────→│                      │                   │
   │                      │ 查 auth_users 验证     │                   │
   │                      │ 生成 32 字节随机 code   │                   │
   │                      │ code → Redis           │                   │
   │                      │ SET code:{v} user_id   │                   │
   │                      │ EX 300 (5 min TTL)     │                   │
   │  {code}              │                      │                   │
   │←─────────────────────│                      │                   │
   │                      │                      │                   │
   │ POST /api/tokens/access                     │                   │
   │ {code, grant_type,   │                      │                   │
   │  client_id,          │                      │                   │
   │  client_secret}      │                      │                   │
   │─────────────────────────────────────────────→│                   │
   │                      │                      │ GET code:{v}      │
   │                      │                      │ 验证 client_id/secret   │
   │                      │                      │ 从 clients.json    │
   │                      │                      │ 签发 access_token   │
   │                      │                      │ 签发 refresh_token  │
   │                      │                      │ DEL code:{v}       │
   │                      │                      │ SET blacklist 标记   │
   │  {access_token,      │                      │                   │
   │   refresh_token,     │                      │                   │
   │   expires_in,        │                      │                   │
   │   token_type}        │                      │                   │
   │←─────────────────────────────────────────────│                   │
   │                      │                      │                   │
   │ GET /api/users/me    │                      │                   │
   │ Authorization:       │                      │                   │
   │   Bearer <AT>        │                      │                   │
   │──────────────────────────────────────────────────────────────────→│
   │                      │                      │ ① 验签(RSA公钥)    │
   │                      │                      │ ② 查Redis黑名单    │
   │                      │                      │ ③ Consul找user-svc │
   │                      │                      │ ④ 转发请求         │
   │  {user_info}         │                      │                   │
   │←──────────────────────────────────────────────────────────────────│
```

### Token 刷新流程

```
 客户端              token-server(Rust)
   │                      │
   │ POST /api/tokens/refresh
   │ {refresh_token,       │
   │  client_id,           │
   │  client_secret}       │
   │──────────────────────→│
   │                      │ 验证 refresh_token 签名
   │                      │ 查 Redis: refresh family 已被轮换？
   │                      │ 签发新 access_token
   │                      │ 签发新 refresh_token (轮换)
   │                      │ SET refresh:{new_jti}
   │                      │ 标记旧 refresh_token 失效
   │                      │
   │  {新的 token 对}       │
   │←──────────────────────│
```

### 用户注销流程

```
 客户端              gateway(Rust)     token-server(Rust)    Redis
   │                      │                   │                │
   │ POST /api/auth/logout│                   │                │
   │ Authorization:       │                   │                │
   │   Bearer <AT>        │                   │                │
   │──────────────────────│                   │                │
   │                      │ 不验证 token       │                │
   │                      │ (白名单)           │                │
   │                      │──────────────────→│                │
   │                      │                   │ 提取 JWT jti    │
   │                      │                   │ 取 JWT exp      │
   │                      │                   │ TTL = exp - now │
   │                      │                   │───────────────→│
   │                      │                   │ SET blacklist:{jti} 1
   │                      │                   │ EX <TTL秒>       │
   │                      │                   │                │
   │ 200 OK               │                   │                │
   │←─────────────────────│                   │                │
```

---

## 4. 数据模型

### 4.1 auth-service — 表前缀 `auth_`

**auth_users** (用户认证信息)

| 列 | 类型 | 说明 |
|----|------|------|
| id | BIGINT PK AUTO_INCREMENT | |
| username | VARCHAR(64) UNIQUE NOT NULL | 登录用户名 |
| password_hash | VARCHAR(255) NOT NULL | argon2/bcrypt 哈希 |
| status | ENUM('active','disabled','deleted') DEFAULT 'active' | |
| created_at | DATETIME DEFAULT NOW() | |
| updated_at | DATETIME DEFAULT NOW() ON UPDATE NOW() | |

```sql
CREATE TABLE auth_users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_username (username),
    INDEX idx_status (status)
);
```

### 4.2 user-service — 表前缀 `usr_`

**usr_users** (用户资料)

| 列 | 类型 | 说明 |
|----|------|------|
| id | BIGINT PK | 等于 auth_users.id |
| display_name | VARCHAR(64) NOT NULL | |
| email | VARCHAR(128) | |
| avatar_url | VARCHAR(512) | |
| department | VARCHAR(128) | |
| title | VARCHAR(64) | |
| employee_id | VARCHAR(32) | |
| status | ENUM('active','disabled','deleted') DEFAULT 'active' | |
| created_at | DATETIME DEFAULT NOW() | |
| updated_at | DATETIME DEFAULT NOW() ON UPDATE NOW() | |

```sql
CREATE TABLE usr_users (
    id BIGINT PRIMARY KEY,
    display_name VARCHAR(64) NOT NULL,
    email VARCHAR(128) DEFAULT '',
    avatar_url VARCHAR(512) DEFAULT '',
    department VARCHAR(128) DEFAULT '',
    title VARCHAR(64) DEFAULT '',
    employee_id VARCHAR(32) DEFAULT '',
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_status (status),
    INDEX idx_department (department)
);
```

**usr_roles** (角色)

| 列 | 类型 | 说明 |
|----|------|------|
| id | BIGINT PK AUTO_INCREMENT | |
| name | VARCHAR(64) NOT NULL | 如 "admin", "operator" |
| description | VARCHAR(255) | |
| created_at | DATETIME DEFAULT NOW() | |
| updated_at | DATETIME DEFAULT NOW() ON UPDATE NOW() | |

```sql
CREATE TABLE usr_roles (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(64) NOT NULL,
    description VARCHAR(255) DEFAULT '',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE INDEX idx_name (name)
);
```

**usr_permissions** (权限)

| 列 | 类型 | 说明 |
|----|------|------|
| id | BIGINT PK AUTO_INCREMENT | |
| code | VARCHAR(128) UNIQUE NOT NULL | 如 "file:read" |
| name | VARCHAR(64) NOT NULL | 如 "文件读取" |
| description | VARCHAR(255) | |
| created_at | DATETIME DEFAULT NOW() | |
| updated_at | DATETIME DEFAULT NOW() ON UPDATE NOW() | |

```sql
CREATE TABLE usr_permissions (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    code VARCHAR(128) NOT NULL,
    name VARCHAR(64) NOT NULL,
    description VARCHAR(255) DEFAULT '',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE INDEX idx_code (code)
);
```

**关联表**

```sql
-- 用户-角色 多对多
CREATE TABLE usr_user_roles (
    user_id BIGINT NOT NULL,
    role_id BIGINT NOT NULL,
    PRIMARY KEY (user_id, role_id),
    INDEX idx_role_id (role_id)
);

-- 角色-权限 多对多
CREATE TABLE usr_role_permissions (
    role_id BIGINT NOT NULL,
    permission_id BIGINT NOT NULL,
    PRIMARY KEY (role_id, permission_id),
    INDEX idx_permission_id (permission_id)
);
```

### 4.3 token-server — JSON文件 + Redis

**clients.json** (自动热重载)

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
    },
    {
      "client_id": "web-console",
      "client_secret_hash": "$argon2id$...",
      "name": "Web管理后台",
      "permissions": ["token:issue", "api_key:manage"],
      "grant_types": ["authorization_code", "refresh_token"],
      "access_token_ttl_sec": 7200,
      "refresh_token_ttl_sec": 604800,
      "status": "active"
    }
  ]
}
```

**Redis 键**

| 键模式 | 值 | TTL | 用途 |
|--------|----|-----|------|
| `code:{code_value}` | `{user_id, client_id, scope}` | 300s | OAuth2 授权码 |
| `blacklist:{jti}` | `1` | access_token 剩余有效期 | 注销黑名单 |
| `refresh:{jti}` | `1` | refresh_token 剩余有效期 | 防 refresh 重放 |
| `api_key:{key_id}` | `{rate_limit_data}` | 永久或按需 | API Key 限流计数 |

### 4.4 JWT Claims 结构

**Access Token**

```json
{
  "sub": "1",                  // 用户ID
  "jti": "uuid-v4",           // 唯一 token ID
  "iss": "superpowers-token-server",
  "iat": 1752230000,
  "exp": 1752233600,          // 默认 1 小时
  "scope": "access",
  "client_id": "web-console",
  "permissions": [            // 用户拥有的权限 codes
    "file:read",
    "file:write",
    "workorder:read"
  ]
}
```

**Refresh Token**

```json
{
  "sub": "1",
  "jti": "uuid-v4",
  "iss": "superpowers-token-server",
  "iat": 1752230000,
  "exp": 1754822000,          // 默认 30 天
  "scope": "refresh",
  "family": "uuid-v4"         // token 家族 ID（轮换防重放）
}
```

**Resource Token**

```json
{
  "sub": "1",
  "jti": "uuid-v4",
  "iss": "superpowers-token-server",
  "iat": 1752230000,
  "exp": 1752230300,          // 短期，如 5 分钟
  "scope": "resource",
  "resource_type": "contract",
  "resource_id": "CT-2025-0001",
  "permission": "read"
}
```

---

## 5. API 接口定义

### 5.1 auth-service (Go)

**依赖:** MySQL/MSSQL/DM（用户表）+ Redis（存储 authorization code，与 token-server 共用同一 Redis 实例）

| 方法 | 路径 | 说明 | JWT验证 |

| 方法 | 路径 | 说明 | JWT验证 |
|------|------|------|---------|
| POST | `/api/auth/register` | 用户注册 | 否 |
| POST | `/api/auth/login` | 密码登录，返回 authorization code | 否 |
| POST | `/api/auth/logout` | 注销 token | 否（白名单） |

**POST /api/auth/login**

Request:
```json
{
  "username": "zhangsan",
  "password": "123456"
}
```

Response:
```json
{
  "code": "a8f3c9e1b2d4...",
  "expires_in": 300
}
```

### 5.2 token-server (Rust)

| 方法 | 路径 | 说明 | JWT验证 |
|------|------|------|---------|
| POST | `/api/tokens/access` | 用 code 换 token | 否 |
| POST | `/api/tokens/refresh` | 用 refresh_token 换新 token 对 | 否 |
| POST | `/api/tokens/revoke` | 注销 token（入黑名单） | 否（白名单，token-server内部） |
| GET | `/api/tokens/.well-known/jwks.json` | 公钥 JWKS 端点 | 否（gateway 启动时拉取） |
| POST | `/api/api-keys` | 创建 API Key | 是（管理端 token） |
| GET | `/api/api-keys` | 列出 API Keys | 是 |
| DELETE | `/api/api-keys/{id}` | 撤销 API Key | 是 |
| POST | `/api/tokens/resource` | 签发 Resource Token | 是 |

**POST /api/tokens/access**

Request:
```json
{
  "grant_type": "authorization_code",
  "code": "a8f3c9e1b2d4...",
  "client_id": "web-console",
  "client_secret": "<secret>"
}
```

Response:
```json
{
  "access_token": "eyJhbGciOi...",
  "refresh_token": "eyJhbGciOi...",
  "token_type": "Bearer",
  "expires_in": 3600
}
```

**POST /api/tokens/resource**

Request:
```json
{
  "resource_type": "contract",
  "resource_id": "CT-2025-0001",
  "permission": "read",
  "ttl_sec": 300
}
```

Response:
```json
{
  "resource_token": "eyJhbGciOi...",
  "expires_in": 300
}
```

### 5.3 user-service (Go)

| 方法 | 路径 | 说明 | JWT验证 |
|------|------|------|---------|
| GET | `/api/users/me` | 获取当前用户资料 | 是 |
| PUT | `/api/users/me` | 更新当前用户资料 | 是 |
| GET | `/api/users` | 用户列表（分页） | 是（需 admin） |
| GET | `/api/users/{id}` | 单个用户详情 | 是 |
| PUT | `/api/users/{id}` | 更新用户（管理） | 是（需 admin） |
| GET | `/api/roles` | 角色列表 | 是 |
| POST | `/api/roles` | 创建角色 | 是（需 admin） |
| PUT | `/api/roles/{id}` | 更新角色 | 是（需 admin） |
| DELETE | `/api/roles/{id}` | 删除角色 | 是（需 admin） |
| GET | `/api/permissions` | 权限列表 | 是 |
| POST | `/api/users/{id}/roles` | 为用户分配角色 | 是（需 admin） |
| DELETE | `/api/users/{id}/roles/{role_id}` | 移除用户角色 | 是（需 admin） |
| POST | `/api/roles/{id}/permissions` | 为角色分配权限 | 是（需 admin） |

### 5.4 api-gateway 路由配置

**白名单（不验证 JWT）：**

```
/api/auth/*
/api/tokens/*
GET /api/tokens/.well-known/*
```

**默认（验证 JWT + 黑名单）：**

其余 `/api/*` 全部需要携带有效 token。

---

## 6. 技术选型

### 6.1 Rust 服务 (api-gateway + token-server)

| 层面 | 选型 | 理由 |
|------|------|------|
| HTTP 框架 | axum | 生态成熟，tower 中间件生态好 |
| JWT | jsonwebtoken | RSA256 签名/验签 |
| Redis | redis-rs | 黑名单、code 管理 |
| Consul 客户端 | rs-consul | 服务发现 |
| HTTP 转发客户端 | reqwest | 转发到后端微服务 |
| 序列化 | serde + serde_json | Rust 标准 |
| 配置 | config + env | 环境变量/文件配置 |
| 日志 | tracing | 结构化日志 |
| Token 管理 | argon2 | 密码/client_secret 哈希 |
| UUID | uuid | jti 生成 |

### 6.2 Go 服务 (auth-service + user-service)

| 层面 | 选型 | 理由 |
|------|------|------|
| HTTP 框架 | gin | 企业主流，生态成熟 |
| 数据库 | GORM | 支持 MySQL/MSSQL/DM（适配器模式） |
| 密码哈希 | argon2id | 安全标准 |
| Redis | go-redis | 授权码管理 |
| JWT | golang-jwt | 与 Rust 互通 |
| 配置 | viper | 多格式配置 |
| 日志 | zap | 高性能结构化日志 |

### 6.3 数据库适配方案

Go 通过 `dbType` 配置切换：

```go
type DBConfig struct {
    Type     string // "mysql", "mssql", "dm"
    Host     string
    Port     int
    User     string
    Password string
    Database string
}

func NewDB(cfg DBConfig) (*gorm.DB, error) {
    switch cfg.Type {
    case "mysql":
        // gorm mysql driver
    case "mssql":
        // gorm sqlserver driver
    case "dm":
        // gorm dm driver (达梦驱动)
    }
}
```

表前缀通过 GORM 的 `TableName()` 方法或全局配置注入。

### 6.4 基础设施

| 组件 | 版本 | 用途 |
|------|------|------|
| Nginx | latest-alpine | 反向代理 |
| Redis | 7-alpine | 黑名单 + code + 限流 |
| Consul | 1.20 | 服务注册与发现 |
| Docker Compose | v3.8+ | 编排 |

---

## 7. 项目目录结构

```
superpowers/
├── Cargo.workspace.toml
├── docker-compose.yml
├── Makefile / Justfile
│
├── digua_common/                     # Rust 共享 crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── token.rs            # JWT Claims 结构体
│       ├── error.rs            # 统一错误码
│       └── config.rs           # 共享配置结构
│
├── api-gateway/                # Rust · axum
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── discovery.rs        # Consul 服务发现
│       ├── jwt.rs              # JWT 验签 + 黑名单
│       ├── router.rs           # 路由表 + 转发
│       └── middleware/
│           ├── mod.rs
│           ├── jwt_auth.rs     # 主鉴权中间件
│           └── rate_limit.rs   # API Key 限流
│
├── token-server/               # Rust · axum
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── models.rs
│       ├── issuer.rs           # JWT 签发
│       ├── store/
│       │   ├── mod.rs
│       │   ├── client.rs       # JSON 文件 client 管理
│       │   └── redis.rs        # Redis 操作
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── access.rs       # POST /tokens/access
│       │   ├── refresh.rs      # POST /tokens/refresh
│       │   ├── revoke.rs       # POST /tokens/revoke
│       │   ├── resource.rs     # POST /tokens/resource
│       │   ├── api_key.rs      # CRUD /api-keys
│       │   └── jwks.rs         # GET .well-known/jwks.json
│       └── middleware/
│           └── auth.rs
│
├── auth-service/               # Go · gin
│   ├── go.mod
│   ├── main.go
│   ├── config/
│   │   └── config.go           # 配置 + dbType 切换
│   ├── models/
│   │   └── user.go             # auth_users 表
│   ├── store/
│   │   └── db.go               # GORM 数据库初始化
│   ├── handlers/
│   │   ├── register.go
│   │   ├── login.go
│   │   └── logout.go
│   └── middleware/
│       └── cors.go
│
├── user-service/               # Go · gin
│   ├── go.mod
│   ├── main.go
│   ├── config/
│   │   └── config.go
│   ├── models/
│   │   ├── user.go             # usr_users
│   │   ├── role.go             # usr_roles
│   │   └── permission.go       # usr_permissions
│   ├── store/
│   │   └── db.go
│   ├── handlers/
│   │   ├── user.go
│   │   ├── role.go
│   │   └── permission.go
│   └── middleware/
│       └── cors.go
│
├── nginx/
│   └── nginx.conf
│
├── consul/
│   └── config.json
│
├── frontend/
│   └── index.html
│
├── Dockerfile.api-gateway
├── Dockerfile.token-server
├── Dockerfile.auth-service
├── Dockerfile.user-service
└── clients.json                # token-server client 配置
```

---

## 8. 部署拓扑 (docker-compose.yml)

```
服务:
  nginx            :80        → / 静态, /api/* → gateway:3000
  api-gateway      :3000      → Consul 发现 → 转发
  token-server     :4000      → Redis + clients.json
  auth-service     :5000      → MySQL/MSSQL/DM + Redis
  user-service     :6000      → MySQL/MSSQL/DM
  redis            :6379
  consul           :8500
  frontend         静态文件挂载

网络:
  superpowers-net (bridge)
```

---

## 9. 安全设计要点

| 层 | 措施 |
|----|------|
| Token 签名 | RSA256，私钥在 token-server，公钥发布于 JWKS 端点 |
| Client Secret | argon2id 哈希存储，不在日志中出现 |
| Authorization Code | 32 字节随机，Redis 5分钟 TTL，一次性使用后删除 |
| Refresh Token 轮换 | 每次刷新都轮换，旧 token 立即标记失效 |
| 密码存储 | argon2id 哈希，加盐 |
| API Key | SHA256 哈希存储 |
| 注销 | jti 入 Redis 黑名单，TTL = token 剩余有效期 |
| CORS | 仅允许已知 origin |

---

## 10. 待定事项 / 后续迭代

- [ ] 手机验证码登录（auth-service 扩展）
- [ ] 微信 OAuth2 登录（auth-service 扩展）
- [ ] 企业 SSO / LDAP 集成
- [ ] API Key 请求速率限制（gateway 中间件）
- [ ] 审计日志（各服务独立或统一采集）
- [ ] Swagger / OpenAPI 文档
- [ ] gRPC 内部通信（token-server ↔ gateway 服务发现）
- [ ] 前端管理页面（用户/角色/权限/API Key 管理）
- [ ] TLS/HTTPS 证书管理
