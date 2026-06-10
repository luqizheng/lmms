# 微服务间 JWT 验证模式设计

> 设计日期: 2026-06-10
> 状态: Draft → 待审核
> 关联: `2025-07-11-token-server-api-gateway-design.md`、`2026-06-10-auth-login-security-design.md`

---

## 1. 概述

明确 coder-micro-services 在以下三种边界上的身份验证模式：

1. **外部客户端 → api-gateway**：用户 JWT 离线验签（已实现）
2. **api-gateway → 下游 Go 服务**：JWT 透传 + 下游解析 user 身份 + 业务级授权
3. **服务对服务直接调用**（未来）：client_credentials 签发 service-token，scope + aud 控制

核心原则：
- **用户身份 JWT**：网关是唯一信任锚，下游不验签，只 decode
- **服务身份 token**：跨服务信任，必须验签 + aud + scope
- **业务级授权**：每个 handler 按 role + dept + permission 自行判定，授权逻辑不进通用中间件

---

## 2. 总体架构

```
   客户端
     │  Authorization: Bearer <user-jwt>
     ▼
  api-gateway (Rust)              ← 唯一入口
     │  ① RSA 验签 + Redis 黑名单
     │  ② 透传 Authorization
     │  ③ 注入 X-User-* 冗余头
     ▼
  下游 Go 服务 (auth / user / contract / ...)
     │  ④ 从 Authorization decode JWT payload
     │  ⑤ 业务层按 role + dept + permission 授权
     │
     │  [未来] 服务对服务调用
     │  service-a ──client_credentials──> token-server
     │  service-a ──Bearer <service-token>──> service-b
     │  service-b 验签 + 检查 aud + 检查 scope
```

---

## 3. 用户身份层（JWT 透传 + 下游解析）

### 3.1 api-gateway 侧

**现有逻辑不变**：
- RSA 离线验签（`services/api-gateway/src/jwt.rs`）
- Redis 黑名单查询

**追加动作**：验签成功后，把 claims 写入冗余 header，便于下游审计、灰度或失败回退。

| Header | 来源 claim | 示例 |
|---|---|---|
| `X-User-Id` | `sub` | `1003` |
| `X-User-Dept` | `dept_id` | `42` |
| `X-User-Dept-Leader` | `is_dept_leader` | `true` |
| `X-User-Roles` | `roles` (逗号分隔) | `sales,dept_leader` |
| `X-User-Perms` | `permissions` (逗号分隔) | `contract.read,contract.write` |
| `X-User-Client` | `client_id` | `web-spa` |

**Authorization 头原样透传**（下游以它为准）。

### 3.2 下游 Go 服务侧

**库**：`github.com/golang-jwt/jwt/v5`

**解析方式**：**decode payload only，不做密码学验签**。原因：
- 用户明确要求"下游不验证 jwt 是否合法"
- 验签意味着每个服务加载 RSA 公钥 + 每次请求 CPU 开销
- 安全前提：下游只监听内网，Authorization 必来自 api-gateway

**中间件** `AuthContextMiddleware`：

```go
// 伪代码，仓库实现时落到 libs/go-jwt-ctx/ 共享包
func AuthContextMiddleware() gin.HandlerFunc {
  return func(c *gin.Context) {
    h := c.GetHeader("Authorization")
    if !strings.HasPrefix(h, "Bearer ") {
      c.AbortWithStatusJSON(401, gin.H{"error": "missing_token"}); return
    }
    token := strings.TrimPrefix(h, "Bearer ")
    claims, err := decodeUnverified(token) // base64 解码 payload，不验签
    if err != nil {
      c.AbortWithStatusJSON(401, gin.H{"error": "invalid_token"}); return
    }
    if time.Now().Unix() > claims.Exp {
      c.AbortWithStatusJSON(401, gin.H{"error": "token_expired"}); return
    }
    principal := &Principal{
      UserID:       claims.Sub,
      DepartmentID: claims.DeptID,
      IsDeptLeader: claims.IsDeptLeader,
      Roles:        claims.Roles,
      Permissions:  toSet(claims.Permissions),
    }
    c.Set("user", principal)
    c.Next()
  }
}
```

**handler 使用**：

```go
func GetCurrentUser(s *Store) gin.HandlerFunc {
  return func(c *gin.Context) {
    user := principal.From(c) // c.MustGet("user").(*Principal)
    record, _ := s.FindByID(c, user.UserID)
    c.JSON(200, record)
  }
}
```

### 3.3 安全前提（部署规范）

- 下游服务**只监听内网**，nginx / docker network / k8s NetworkPolicy 兜底
- api-gateway 是 Authorization header 唯一来源
- 一旦下游误暴露公网，攻击者即可伪造身份——靠**网络层 + 配置审计**兜底，**不**靠 JWT 验签

---

## 4. 下游业务授权

### 4.1 原则

- 授权不进通用中间件
- 每个 handler 内部按业务规则判定
- 提供可复用的 `authz` 辅助函数包

### 4.2 典型规则（合同服务示例）

```go
// libs/go-authz/contract.go
func CanReadContract(user *Principal, c *Contract) bool {
  if c.OwnerID == user.UserID { return true }                              // owner
  if user.HasPermission("contract.read.all") { return true }               // 超管
  if user.IsDeptLeader && c.DeptID == user.DepartmentID { return true }    // 本部门主管
  if c.Visibility == "dept" && c.DeptID == user.DepartmentID { return true } // 部门可见
  return false
}

func CanWriteContract(user *Principal, c *Contract) bool {
  if c.OwnerID == user.UserID { return true }
  if user.HasPermission("contract.write.all") { return true }
  return false
}
```

### 4.3 字段脱敏

```go
func GetContract(s *Store) gin.HandlerFunc {
  return func(c *gin.Context) {
    user := principal.From(c)
    contract, _ := s.FindByID(c.Param("id"))
    if contract == nil {
      c.JSON(404, gin.H{"error": "not_found"}); return
    }
    if !authz.CanReadContract(user, contract) {
      c.JSON(403, gin.H{"error": "forbidden"}); return
    }
    // 字段级授权：普通员工看不到价格
    if !user.HasPermission("contract.read.price") {
      contract.Price = nil
    }
    c.JSON(200, contract)
  }
}
```

### 4.4 一致性

- 角色-权限关系由 user-service 维护
- 登录时一次写入 JWT，下游不查 DB
- 权限变更要等下次签发 JWT 才生效（接受，最终一致性）
- JWT 默认 TTL 由 token-server 配置（参考 `2026-06-10-auth-login-security`）

---

## 5. 服务对服务信任（client_credentials + service-token）

### 5.1 适用场景

未来 service-a 需要直接调 service-b 内部接口（不经 api-gateway 转发给用户，或虽经网关但 service-b 需要识别调用方服务身份）。

### 5.2 流程

```
service-a
  │ POST /api/tokens/access
  │   grant_type=client_credentials
  │   client_id=service-a
  │   client_secret=***
  │   scope=contract.read contract.write
  ▼
token-server
  │ 查 clients.json 校验 client_secret
  │ 签发 service-token (RS256 JWT)
  │ claims: {
  │   sub: "service-a",
  │   scope: "contract.read contract.write",
  │   aud: "contract-service",
  │   token_type: "service",
  │   exp: now + 1h
  │ }
  ▼
service-a → contract-service
  │ Authorization: Bearer <service-token>
  │ X-Service-Caller: service-a
  ▼
contract-service
  │ ① RSA 验签（与用户 JWT 同密钥对）
  │ ② 检查 aud == "contract-service"
  │ ③ 检查 token_type == "service"
  │ ④ 检查 scope ⊇ 本操作所需 scope
```

### 5.3 与用户 JWT 的关键差异

| 维度 | 用户 JWT | service-token |
|---|---|---|
| 验签 | 网关验，下游不验 | **下游必须验签** |
| `aud` | 可选 | 必填，限定目标服务 |
| `scope` | 标识 grant 类型 | 限定允许的操作 |
| `token_type` claim | 不存在 | `"service"` |
| TTL | 短（15-60min） | 中（1h） |
| 缓存 | 客户端持有 | service 本地缓存 |

### 5.4 客户端注册

新增微服务必须先在 token-server 注册，否则拿不到 service-token。

`docker/config/token-server/clients.json`：
```json
{
  "service-a": {
    "secret": "<bcrypt or HMAC>",
    "allowed_scopes": ["contract.read", "contract.write"],
    "allowed_aud": ["contract-service"],
    "enabled": true
  },
  "service-b": {
    "secret": "<...>",
    "allowed_scopes": ["user.read"],
    "allowed_aud": ["user-service"],
    "enabled": true
  }
}
```

token-server 在签发时**主动校验**：
- 请求 scope 必须是 `allowed_scopes` 子集
- 不允许为 service-a 签发 `aud=user-service` 的 token

### 5.5 service-token 缓存

- service 本地缓存（含过期前 60s 余量），避免每请求换 token
- 缓存键：`{client_id} -> {token, exp}`
- 进程内 LRU 即可，**不**放 Redis

### 5.6 下游验证中间件

```go
// 服务侧验证 service-token
func ServiceAuthMiddleware() gin.HandlerFunc {
  return func(c *gin.Context) {
    h := c.GetHeader("Authorization")
    claims, err := verifyRS256(h, publicKey) // 必须验签
    if err != nil { c.AbortWithStatusJSON(401, ...); return }
    if claims.Audience != expectedService {
      c.AbortWithStatusJSON(403, gin.H{"error": "wrong_audience"}); return
    }
    if claims.TokenType != "service" {
      c.AbortWithStatusJSON(401, gin.H{"error": "wrong_token_type"}); return
    }
    c.Set("service", claims.Sub)
    c.Set("scopes", toSet(claims.Scope))
    c.Next()
  }
}
```

---

## 6. JWT Claims 字段规范

### 6.1 用户 JWT

**claim schema 由 user-service 决定，token-server 不做假设**。token-server 只负责：注入标准声明（`sub` / `iss` / `aud` / `exp` / `iat` / `jti`）+ 透传 user-service 给的自定义 claims。

| claim | 来源 | 必填 | 说明 |
|---|---|---|---|
| `sub` | token-server | ✓ | 用户 ID（数字字符串，由调用方传入） |
| `iss` | token-server | ✓ | `"superpowers-token-server"` |
| `aud` | token-server | | 可选 |
| `exp` | token-server | ✓ | 过期时间戳 |
| `iat` | token-server | ✓ | 签发时间戳 |
| `jti` | token-server | ✓ | UUID |
| `client_id` | token-server | ✓ | 签发方客户端 ID |
| `scope` | token-server | ✓ | `"access"` / `"refresh"` / `"resource"` |
| `permissions` | user-service | ✓ | 细粒度权限点 |
| **`dept_id`** | user-service | ✓（access） | 部门 ID |
| **`is_dept_leader`** | user-service | ✓（access） | 是否部门主管 |
| **`roles`** | user-service | ✓（access） | 粗粒度角色 |
| **其他业务字段** | user-service | | user-service 决定 schema，加新字段无需改 token-server |

**关键设计**：
- `issue_access_token` 签名改为接受 `extra_claims: serde_json::Value`（或 `HashMap<String, Value>`），原样合并进 JWT custom claims
- 登录时调用链：`auth-service` → `user-service.GetClaims(user_id, client_id)` → 返回完整 claims 字典 → `token-server` 签名
- user-service 暴露内部端点 `GET /internal/users/{id}/claims`（仅内网，**不**经 api-gateway），由 auth-service / token-server 在签 token 前调用
- 下游 Go 服务解析 claims 时不假设 schema，用通用 `Principal` 结构 + 业务字段按需读取
- user-service 是用户身份的真实源，加新字段只改 user-service + 下游消费方，不动 token-server

### 6.2 service-token

| claim | 类型 | 必填 | 说明 |
|---|---|---|---|
| `sub` | string | ✓ | 服务客户端 ID（"service-a"） |
| `iss` | string | ✓ | token-server |
| `aud` | string | ✓ | 目标服务名（"contract-service"） |
| `exp` | int64 | ✓ | |
| `iat` | int64 | ✓ | |
| `jti` | string | ✓ | UUID |
| `scope` | string | ✓ | 空格分隔的操作集合 |
| `token_type` | string | ✓ | 固定 `"service"` |

---

## 7. 错误处理

| 场景 | HTTP | error code |
|---|---|---|
| 缺 Authorization | 401 | `missing_token` |
| JWT base64 解析失败 | 401 | `invalid_token` |
| exp 已过 | 401 | `token_expired` |
| 业务规则拒绝 | 403 | `forbidden` |
| service-token 验签失败 | 401 | `invalid_token` |
| service-token aud 不匹配 | 403 | `wrong_audience` |
| service-token scope 不足 | 403 | `insufficient_scope` |
| service-token 类型错 | 401 | `wrong_token_type` |

**下游不查黑名单**（黑名单由网关查），但**解析 exp** 判断过期。

---

## 8. 测试

### 8.1 单元

- `libs/go-jwt-ctx/`：decode 成功/失败/过期/缺字段
- `libs/go-authz/`：owner / 同部门 / 部门主管 / 超管 / 默认拒绝矩阵

### 8.2 集成

- 新增 `tests/specs/contract-service/` 端到端 spec
- 覆盖：未登录 401 / 普通用户 403 / owner 200 / 部门主管跨部门 403 / 超管 200 / 字段脱敏

### 8.3 E2E

- 起 4 服务，签发三种身份的 JWT
- 模拟外部用户访问合同接口
- 模拟 service-a → contract-service 调内部接口

---

## 9. 待办 / 风险

1. **claims 字段命名**：本设计已定 `dept_id` / `is_dept_leader` / `roles`，需 user-service 配合在登录时提供这些信息
2. **网络隔离规范**：需在 docker-compose / 部署文档明确下游服务端口不暴露公网
3. **token-server 修改影响面**：`issue_access_token` 签名变更，需同步更新现有集成测试
4. **service-token 引入期**：先实现用户身份层，服务对服务等出现实际需求时再做

---

## 10. 改动清单（实现阶段）

| 路径 | 改动 |
|---|---|
| `services/user-service/handlers/claims.go` (新建) | 实现 `GET /internal/users/{id}/claims` 端点，聚合 user + dept + roles + permissions 为 claims JSON |
| `services/user-service/routes.go` | 注册 `/internal/...` 路由组，限内网（不挂 api-gateway） |
| `services/token-server/src/issuer.rs` | `issue_access_token` 改为接受 `extra_claims: serde_json::Value`，原样合并；不再硬编码 dept_id / roles 字段 |
| `services/token-server/src/routes/access.rs` | 签 access 前 `reqwest` 调 user-service 拿 claims，调失败则返回 503 |
| `services/api-gateway/src/middleware/mod.rs` | 验签后注入 X-User-* 头（按 `permissions` / `roles` / `dept_id` / `is_dept_leader` 通用化，不假设 schema） |
| `libs/go-jwt-ctx/` (新建) | Go 共享包：JWT decode + 通用 Principal + gin middleware |
| `libs/go-authz/` (新建) | Go 共享包：CanRead/CanWrite/CanDelete 辅助函数（基于通用 Principal 字段） |
| `services/auth-service/go.mod` 等 | 引入 `libs/go-jwt-ctx` 并接入 middleware |
| `services/user-service/go.mod` 等 | 同上 |
| `docker/config/token-server/clients.json` | 预留 service-* 客户端结构 |
| `docs/guides/microservice-auth.md` (新建) | 部署规范：端口隔离、网关唯一入口 |
