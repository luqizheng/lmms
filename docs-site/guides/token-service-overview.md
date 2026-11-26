# Token 管理系统架构总览

> 本文档面向开发者与 AI 协作者，说明 token-service 的设计、使用方式及各服务间的令牌流转关系。

## TL;DR

- **token-service**（compose host 端口 8082 / 容器内 8082）是 OAuth 2.0 令牌管理服务，负责 JWT 签发、JWKS 公钥分发、OAuth 客户端管理与 API Key 管理。
- **iam-service** 已合并部分 token 管理能力，用户登录/注册/刷新走 `/iam/auth/*`，不再经过 token-service。
- **api-gateway** 依赖 token-service 的 JWKS 验签所有请求，是 token-service 最核心的消费者。
- 其他服务（notification / audit / config / file）不直接调用 token-service，通过 gateway JWT 验签间接依赖。

---

## 1. 服务定位

token-service 是 Rust + Axum 构建的独立微服务，是系统中 **JWT 签发的单一真相源**。

| 属性 | 值 |
|---|---|
| 端口 | 8082（compose host = 容器内；直连本机：`http://localhost:8082`） |
| 技术栈 | Rust / Axum 0.8 / SQLx / RS256 |
| 数据库 | MySQL（共享 `superpowers` 库，表前缀 `token_`） |
| 缓存 | Redis（黑名单） |

---

## 2. API 端点

### 2.1 令牌签发

| 端点 | 方法 | 鉴权 | 说明 |
|---|---|---|---|
| `/tokens/access` | POST | `public` | OAuth 2.0 授权码换 token（client_id + client_secret + code） |
| `/tokens/refresh` | POST | `public` | 用 refresh_token 换新 access_token（单次使用，防重放） |
| `/tokens/revoke` | POST | `public` | 撤销 token，jti 写入 Redis 黑名单 |
| `/tokens/resource` | POST | `service-jwt` | 签发受限资源令牌（自定义 TTL + scope；aud=token-service 准入，WP-LMMS-HARDEN-01-R1 收紧） |
| `/tokens/user-jwt` | POST | `service-jwt` | 签发用户身份 JWT（由 iam-service 内部转发调用） |
| `/tokens/service-jwt` | POST | `bootstrap token` | 签发服务间通信 JWT（1h 有效期） |

### 2.2 密钥与 OAuth 管理

| 端点 | 方法 | 鉴权 | 说明 |
|---|---|---|---|
| `/tokens/.well-known/jwks.json` | GET | `public` | JWKS 公钥分发（api-gateway / iam-service 拉取验证 JWT） |
| `/api-keys` | POST / GET | `bearer` | API Key 创建 / 列表（OAuth 客户端的简化入口） |
| `/admin/oauth-clients` | POST / GET | `bearer` | OAuth 客户端完整 CRUD |
| `/admin/oauth-clients/{id}` | PUT / DELETE | `bearer` | 更新 / 删除客户端 |
| `/admin/oauth-clients/{id}/reset-secret` | POST | `bearer` | 重置 secret，返回新明文 |

---

## 3. 数据库表

全部表以 `token_` 前缀，存储在共享 MySQL `superpowers` 库中：

```sql
-- 1) OAuth 客户端注册表
CREATE TABLE token_oauth_clients (
  client_id             VARCHAR(64)  PRIMARY KEY,
  client_secret_hash    VARCHAR(255) NOT NULL,   -- Argon2 哈希存储
  name                  VARCHAR(128),
  permissions           JSON,                    -- ['token:issue', 'user:read', ...]
  grant_types           JSON,                    -- ['authorization_code', 'client_credentials']
  access_token_ttl_sec  BIGINT,
  refresh_token_ttl_sec BIGINT,
  status                VARCHAR(16),             -- 'active' / 'disabled'
  ...
);

-- 2) 授权码（OAuth 授权码模式用）
CREATE TABLE token_auth_codes (
  code        VARCHAR(128) PRIMARY KEY,
  user_id     VARCHAR(64),
  expires_at  TIMESTAMP,
  consumed_at TIMESTAMP NULL,     -- NULL=未消费，消费后填时间
  ...
);

-- 3) 吊销名单（token 撤销后 jti 存此表）
CREATE TABLE token_revoked_jtis (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMP,
  ...
);

-- 4) refresh 标记（防 refresh_token 重放）
CREATE TABLE token_refresh_markers (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMP,
  ...
);
```

---

## 4. OAuth 客户端（oauth-clients）详解

### 4.1 概念

每个 OAuth 客户端（即 API Key / OAuth Client）是一个可以调用 token-service 签发令牌的**受信实体**。管理员通过 API 创建客户端后，返回一次性明文的 `client_id` + `client_secret`，调用方用这对凭证换取 JWT。

### 4.2 默认客户端（seed）

数据库首次启动时空表，自动插入 4 个预置客户端：

| client_id | 名称 | 权限 | grant_types |
|---|---|---|---|
| `api-gateway` | API Gateway | `token:issue`, `token:revoke` | `authorization_code`, `refresh_token` |
| `token-service` | Token Service | `token:issue`, `token:revoke`, `token:refresh`, `user:read` | `authorization_code`, `refresh_token` |
| `user-service` | User Service | `user:read`, `user:write`, `role:read`, `permission:read` | `client_credentials` |
| `notification-service` | Notification Service | `notify:send`, `notify:read` | `client_credentials` |

> 开发环境默认 secret 明文为 `dev-gateway-secret`。生产环境需通过 API 创建新客户端并禁用默认客户端。

### 4.3 三种使用模式

#### 模式一：OAuth 授权码流程（authorization_code）

```
前端用户 → 获取授权码 → POST /tokens/access { grant_type:"authorization_code", code, client_id, client_secret }
                              │
                    token-service:
                      1. 验证 client_id + client_secret（Argon2 对比 hash）
                      2. 检查客户端 status=active
                      3. 原子消费授权码（token_auth_codes 表, SET consumed_at）
                      4. 向 iam-service 查询用户 claims（/internal/users/{id}/claims）
                      5. 按客户端配置的 TTL 签发 access_token + refresh_token
```

> **注意**：当前主登录流程已迁至 iam-service（`/iam/auth/login`），OAuth 授权码模式主要用于第三方接入场景。

#### 模式二：客户端凭证模式（client_credentials）

```
外部服务 → POST /tokens/access { grant_type:"client_credentials", client_id, client_secret }
                              │
                    token-service: 验证客户端，签发 access_token
```

适用于服务间调用，token 的 permissions 来自客户端配置。

#### 模式三：API Key 管理（面向外部开发者）

```bash
# 管理员创建 API Key（唯一一次返回明文 secret）
curl -X POST /v1/api-keys -H "Authorization: Bearer <admin-token>" \
  -d '{"name":"mobile-app", "permissions":["read"], "grant_types":["client_credentials"]}'

# 外部开发者用 API Key 换取 access token
curl -X POST /v1/tokens/access \
  -d '{"grant_type":"client_credentials", "client_id":"xxx", "client_secret":"<返回的secret>"}'
```

### 4.4 Secret 安全机制

- 创建时返回明文 `client_secret` → **仅此一次**，之后数据库中只存 Argon2 哈希
- 验证时用 Argon2 对比（不是可逆解密）
- 支持 `reset-secret` 重置（返回新明文，旧 secret 即刻失效）
- 开发环境默认 hash 过期时自动从 `dev-gateway-secret` 重新计算

---

## 5. 各服务如何使用 token-service

### 5.1 api-gateway → token-service（最核心的依赖）

```
api-gateway 启动时:
  JWKSCache::new(url = cfg.jwks_url) → GET /tokens/.well-known/jwks.json
  → 缓存公钥 + 定时刷新（3600s）
  → 用公钥验证所有请求的 JWT 签名

运行时：
  用户请求 → gateway auth_middleware:
    1. 提取 Authorization: Bearer <jwt>
    2. 用 JWKSCache 验签（来自 token-service）
    3. 检查 Redis 黑名单（token 是否被 token-service 撤销）
    4. 检查 perm_epoch（权限是否过期）
```

代码路径：`services/api-gateway/src/main.rs:182-192`、`services/api-gateway/src/middleware/mod.rs`

### 5.2 iam-service → token-service（内部转发）

iam-service 持有自己的 JWT 密钥对（`keys/private.pem`），登录/刷新直接自己签发 token。**但是**内部维护了 `TokenForwardClient`，通过 service-jwt 鉴权调用 token-service 的 `/tokens/user-jwt`：

```rust
// services/iam-service/src/service/token_forward.rs
pub async fn issue_user_jwt(&self, user_id, user_code, claims, ttl) -> String {
    let service_jwt = self.service_jwt.get("token-service").await?;
    self.http.post("http://token-service:8082/tokens/user-jwt")
        .bearer_auth(&service_jwt)
        .json(&req).send().await
}
```

iam-service 启动时也从 token-service 拉取 JWKS（`main.rs:45-53`），用于验证自身接收到的 JWT。

### 5.3 其他服务（notification / audit / config / file）

**不直接调用 token-service**。通过以下方式间接依赖：

1. 接收的请求已经过 api-gateway JWT 验证（gateway 用 token-service 的 JWKS 验签）
2. 服务间调用使用 service-jwt（由 token-service 签发，通过 `libs/jwt-ctx::Principal` extractor 取身份）
3. 通过 `libs/jwt-ctx::Principal` extractor 直接获取用户身份，无需关心 token 来源

---

## 6. 架构演变

```
旧架构（token-service 面向用户）：
  用户 → api-gateway → token-service（处理登录+签发token）
                      → user-service（查询用户信息）

当前架构（iam-service 吸收 token 管理）：
  用户 → api-gateway → iam-service（统一处理 登录+用户+角色+权限+token 签发）
          │
          ├── 转发 token 相关请求 → token-service（JWKS + service-jwt + API Key）
          │
          └── JWT 验证依赖 token-service 的 JWKS 公钥
```

**token-service 退化成了底层基础设施**，保留三项职责：
1. **JWKS 公钥分发** — api-gateway 和 iam-service 依赖它验签
2. **service-jwt 签发** — 服务间通信凭证
3. **API Key / OAuth 客户端管理** — 预留第三方接入通道

---

## 7. 配置

### 必需环境变量

| 变量 | 说明 |
|---|---|
| `TOKEN_DATABASE_URL` | MySQL 连接串 |
| `TOKEN_JWT_PRIVATE_KEY_PATH` | RS256 私钥文件路径 |
| `TOKEN_JWT_PUBLIC_KEY_PATH` | RS256 公钥文件路径 |
| `TOKEN_ISSUER` | JWT `iss` claim |
| `TOKEN_USER_SERVICE_URL` | iam-service 地址（查询用户 claims） |
| `SERVICE_BOOTSTRAP_TOKENS` | 逗号分隔的 bootstrap token（service-jwt 签发用） |

### 可选环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `TOKEN_DEFAULT_ACCESS_TTL_SEC` | 3600 | access token 默认 TTL |
| `TOKEN_DEFAULT_REFRESH_TTL_SEC` | 2592000 | refresh token 默认 TTL |
| `TOKEN_SERVER_PORT` | 4000 | 监听端口 |
| `SWAGGER_ENABLED` | true | 是否暴露 Swagger UI |

> 完整环境变量列表见 [token-service 环境变量](./token-service-env)

---

## 8. 相关文档

| 文档 | 说明 |
|---|---|
| [token-service 环境变量](./token-service-env) | 完整 env 列表与默认值 |
| [服务间认证](./service-to-service-auth) | service-jwt 的工作方式 |
| [前端鉴权](./frontend-auth) | 历史参考（PRISM console 已移除；token 交互机制原理仍适用） |
| [API Gateway 路由配置](./api-gateway-routing) | 路由如何转发到 token-service |
| [迁移状态端点](./migrations-state) | token-service 的迁移状态查询 |

---

## 9. 常见问题

**Q: 用户登录为什么不走 token-service？**  
A: iam-service 已经合并了 token-service、credential-login-service、user-service 的功能。登录走 `/v1/iam/auth/login`，由 iam-service 自签发 JWT（有自己的密钥对）。

**Q: 那 token-service 还能用吗？**  
A: 能。api-gateway 依赖它的 JWKS 验签，iam-service 内部通过 `TokenForwardClient` 调用它签发 user-jwt，服务间通信的 service-jwt 也由它签发。

**Q: 如何创建一个新的 API Key？**  
A: `POST /v1/api-keys`（需要 admin token），返回 `api_key_id` + `api_key_secret`（仅此一次）。然后调用方用这对凭证调用 `POST /v1/tokens/access`（grant_type=client_credentials）获取 JWT。

**Q: OAuth 客户端和 API Key 有什么区别？**  
A: 共享同一张 `token_oauth_clients` 表，功能完全相同。`/api-keys` 是简化的创建入口（自动生成 UUID 作为 client_id），`/admin/oauth-clients` 提供完整 CRUD（支持自定义 client_id、状态管理、secret 重置等）。
