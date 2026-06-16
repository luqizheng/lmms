# token-service 特性清单

## 概述

本文件列出 token-service 的所有功能特性，用于 AI 自动化检查和变更追踪。

---

## 核心特性

### 1. OAuth 2.0 Access Token 颁发

| 属性 | 值 |
|------|-----|
| 名称 | access_token_issue |
| 描述 | 基于授权码交换获取访问令牌 |
| 相关文件 | [routes/access.rs](file:///d:/projects/coder-micro-services/services/token-service/src/routes/access.rs) |
| 相关服务 | [service/auth_code_svc.rs](file:///d:/projects/coder-micro-services/services/token-service/src/service/auth_code_svc.rs) |
| 测试覆盖 | [tests/auth_code_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/auth_code_test.rs) |
| 状态 | ✅ 已实现 |

**流程说明：**
- 验证 client_id 和 client_secret
- 使用 Argon2 验证客户端密钥哈希
- 消费授权码获取 user_id
- 调用 user-service 获取用户 claims
- 签发 Access Token 和 Refresh Token

---

### 2. Refresh Token 刷新

| 属性 | 值 |
|------|-----|
| 名称 | refresh_token |
| 描述 | 使用 Refresh Token 获取新的 Access Token，支持防重放攻击 |
| 相关文件 | [routes/refresh.rs](file:///d:/projects/coder-micro-services/services/token-service/src/routes/refresh.rs) |
| 相关服务 | [service/refresh_svc.rs](file:///d:/projects/coder-micro-services/services/token-service/src/service/refresh_svc.rs) |
| 测试覆盖 | [tests/refresh_marker_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/refresh_marker_test.rs) |
| 状态 | ✅ 已实现 |

**安全特性：**
- Refresh Token 单次使用机制
- 使用 refresh_marker 表标记已使用的 token jti
- 防止 token 重放攻击

---

### 3. Token 撤销

| 属性 | 值 |
|------|-----|
| 名称 | token_revoke |
| 描述 | 主动撤销令牌，加入黑名单 |
| 相关文件 | [routes/revoke.rs](file:///d:/projects/coder-micro-services/services/token-service/src/routes/revoke.rs) |
| 相关服务 | [service/revoke_svc.rs](file:///d:/projects/coder-micro-services/services/token-service/src/service/revoke_svc.rs) |
| 测试覆盖 | [tests/revoke_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/revoke_test.rs) |
| 状态 | ✅ 已实现 |

**实现机制：**
- 将 token jti 存入 revoked_jti 表
- 支持指定过期时间自动清理

---

### 4. Resource Token

| 属性 | 值 |
|------|-----|
| 名称 | resource_token |
| 描述 | 为特定资源生成受限访问令牌 |
| 相关文件 | [routes/resource.rs](file:///d:/projects/coder-micro-services/services/token-service/src/routes/resource.rs) |
| 测试覆盖 | ❌ 待添加 |
| 状态 | ✅ 已实现 |

**特性：**
- 支持自定义 TTL（默认 300 秒）
- 支持指定权限范围
- 绑定特定资源类型和资源 ID

---

### 5. User JWT 签发

| 属性 | 值 |
|------|-----|
| 名称 | user_jwt |
| 描述 | 为用户生成身份令牌 |
| 相关文件 | [routes/user_jwt.rs](file:///d:/projects/coder-micro-services/services/token-service/src/routes/user_jwt.rs) |
| 测试覆盖 | ❌ 待添加 |
| 状态 | ✅ 已实现 |

**特性：**
- 支持自定义声明（claims）
- 支持自定义过期时间（默认 3600 秒）

---

### 6. API Key 管理

| 属性 | 值 |
|------|-----|
| 名称 | api_key_management |
| 描述 | 创建 OAuth 客户端凭证（API Key） |
| 相关文件 | [routes/api_key.rs](file:///d:/projects/coder-micro-services/services/token-service/src/routes/api_key.rs) |
| 相关服务 | [service/oauth_client_svc.rs](file:///d:/projects/coder-micro-services/services/token-service/src/service/oauth_client_svc.rs) |
| 测试覆盖 | [tests/oauth_client_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/oauth_client_test.rs) |
| 状态 | ✅ 已实现 |

**特性：**
- 配置权限列表（permissions）
- 配置授权类型（grant_types）
- 配置令牌有效期

---

### 7. JWKS 端点

| 属性 | 值 |
|------|-----|
| 名称 | jwks_endpoint |
| 描述 | 提供公钥 JWKS 格式供验证方获取 |
| 相关文件 | [routes/jwks.rs](file:///d:/projects/coder-micro-services/services/token-service/src/routes/jwks.rs) |
| 测试覆盖 | ❌ 待添加 |
| 状态 | ✅ 已实现 |

**支持格式：**
- PKCS1 格式公钥
- PKCS8 格式公钥

---

### 8. 密钥自动生成工具

| 属性 | 值 |
|------|-----|
| 名称 | keygen |
| 描述 | 生成 RSA 密钥对（私钥和公钥）的独立工具 |
| 相关文件 | [src/bin/keygen.rs](file:///d:/projects/coder-micro-services/services/token-service/src/bin/keygen.rs) |
| 测试覆盖 | ❌ 待添加 |
| 状态 | ✅ 已实现 |

**使用方式：**
```bash
cargo run --bin keygen
```

**输出文件：**
- `keys/private.pem` - RSA 私钥（2048 位）
- `keys/public.pem` - RSA 公钥

**注意事项：**
- 当前需要**手动执行**生成密钥，服务启动时**不会自动生成**
- 如果密钥文件不存在，服务启动会 panic（[main.rs#L52-L55](file:///d:/projects/coder-micro-services/services/token-service/src/main.rs#L52-L55)）

---

## 安全特性

| 特性 | 描述 | 实现位置 |
|------|------|----------|
| Argon2 哈希 | 使用 Argon2 存储客户端密钥哈希 | [secret.rs](file:///d:/projects/coder-micro-services/services/token-service/src/secret.rs) |
| RS256 签名 | JWT 使用非对称加密签名 | [issuer.rs](file:///d:/projects/coder-micro-services/services/token-service/src/issuer.rs) |
| 防重放攻击 | Refresh Token 单次使用机制 | [service/refresh_svc.rs](file:///d:/projects/coder-micro-services/services/token-service/src/service/refresh_svc.rs) |
| 客户端状态管理 | 支持启用/禁用 OAuth 客户端 | [models/oauth_client.rs](file:///d:/projects/coder-micro-services/services/token-service/src/models/oauth_client.rs) |

---

## 服务能力

| 能力 | 描述 | 端点 |
|------|------|------|
| 健康检查 | `/health` | ✅ |
| 就绪检查 | `/ready` | ✅ |
| 指标监控 | `/metrics` (Prometheus) | ✅ |
| API 文档 | `/swagger-ui` | ✅ |
| 服务发现 | Consul 自动注册 | ✅ |

---

## 数据模型

| 模型 | 描述 | 文件 |
|------|------|------|
| AuthCode | 授权码模型 | [models/auth_code.rs](file:///d:/projects/coder-micro-services/services/token-service/src/models/auth_code.rs) |
| OAuthClient | OAuth 客户端模型 | [models/oauth_client.rs](file:///d:/projects/coder-micro-services/services/token-service/src/models/oauth_client.rs) |
| RefreshMarker | Refresh Token 重放标记 | [models/refresh_marker.rs](file:///d:/projects/coder-micro-services/services/token-service/src/models/refresh_marker.rs) |
| RevokedJti | 已撤销令牌记录 | [models/revoked_jti.rs](file:///d:/projects/coder-micro-services/services/token-service/src/models/revoked_jti.rs) |

---

## 测试覆盖状态

| 测试文件 | 覆盖功能 | 状态 |
|----------|----------|------|
| [auth_code_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/auth_code_test.rs) | 授权码消费 | ✅ |
| [oauth_client_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/oauth_client_test.rs) | 客户端管理 | ✅ |
| [refresh_marker_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/refresh_marker_test.rs) | 防重放机制 | ✅ |
| [revoke_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/revoke_test.rs) | Token 撤销 | ✅ |
| [issuer_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/issuer_test.rs) | 令牌签发 | ✅ |
| [integration_test.rs](file:///d:/projects/coder-micro-services/services/token-service/tests/integration_test.rs) | 集成测试 | ✅ |

---

## 配置项

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| server.host | String | 0.0.0.0 | 服务绑定地址 |
| server.port | u16 | 4000 | 服务端口 |
| issuer | String | superpowers-token-service | JWT issuer 标识 |
| database.db_type | String | mysql | 数据库类型 |
| database.url | String | - | 数据库连接 URL |
| database.max_connections | u32 | 16 | 最大连接数 |
| jwt_private_key_path | String | keys/private.pem | 私钥路径 |
| jwt_public_key_path | String | keys/public.pem | 公钥路径 |
| default_access_ttl_sec | i64 | 3600 | Access Token 默认有效期 |
| default_refresh_ttl_sec | i64 | 2592000 | Refresh Token 默认有效期 |
| default_resource_ttl_sec | i64 | 300 | Resource Token 默认有效期 |
| user_service_url | String | http://user-service:8080 | 用户服务地址 |
| consul.addr | String | http://localhost:8500 | Consul 地址 |
| consul.service_name | String | token-service | 服务名称 |

---

## 环境变量

| 变量名 | 作用 | 默认值 |
|--------|------|--------|
| SWAGGER_ENABLED | 启用 Swagger UI | true |
| HEALTH_CHECK_REQUIRED | 要求健康检查端点 | true |
| MIGRATIONS_REQUIRED | 要求数据库迁移 | true |

---

## API 端点清单

| 方法 | 路径 | 功能 |
|------|------|------|
| GET | /health | 健康检查 |
| GET | /ready | 就绪检查 |
| GET | /metrics | Prometheus 指标 |
| GET | /swagger-ui | Swagger UI |
| GET | /api-docs/openapi.json | OpenAPI 规范 |
| POST | /api/tokens/access | 获取 Access Token |
| POST | /api/tokens/refresh | 刷新 Token |
| POST | /api/tokens/revoke | 撤销 Token |
| POST | /api/tokens/resource | 获取 Resource Token |
| GET | /api/tokens/.well-known/jwks.json | JWKS |
| GET | /.well-known/jwks.json | JWKS（别名） |
| POST | /api/api-keys | 创建 API Key |
| POST | /api/v1/tokens/user-jwt | 获取用户 JWT |

---

## 变更日志

| 版本 | 日期 | 变更内容 | 作者 |
|------|------|----------|------|
| 1.0.0 | 2026-06-11 | 初始版本，支持 OAuth 2.0 授权码模式 | - |
