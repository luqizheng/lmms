# 微服务间 JWT 验证模式设计

> 设计日期: 2026-06-10
> 状�? Draft �?待审�?
> 关联: `2025-07-11-token-server-api-gateway-design.md`、`2026-06-10-auth-login-security-design.md`

---

## 1. 概述

明确 coder-micro-services 在以下三种边界上的身份验证模式：

1. **外部客户�?�?api-gateway**：用�?JWT 离线验签（已实现�?
2. **api-gateway �?下游微服务**：JWT 透传 + 下游直接解析 payload（**不再使用 X-User-* headers**）
3. **服务对服务直接调�?*（未来）：client_credentials 签发 service-token，scope + aud 控制

核心原则�?
- **用户身份 JWT**：网关是唯一信任锚，下游不验签，仅 decode payload
- **服务身份 token**：跨服务信任，必须验�?+ aud + scope
- **业务级授�?*：每�?handler �?role + dept + permission 自行判定，授权逻辑不进通用中间�?
- **禁止使用 X-User-* headers**：下游服务必须从 JWT payload 解析用户身份，不得依赖网关注入的 X-User-* headers

---

## 2. 总体架构

```
   客户�?
     �? Authorization: Bearer <user-jwt>
     �?
  api-gateway (Rust)              �?唯一入口
     �? �?RSA 验签 + Redis 黑名�?
     �? �?透传 Authorization（**不注入 X-User-* headers**）
     �?
  下游微服务 (iam-service / notification-service / ...)
     �? �?�?Authorization decode JWT payload（不验签）
     �? �?业务层按 role + dept + permission 授权
     �?
     �? [未来] 服务对服务调�?
     �? service-a ──client_credentials──> token-server
     �? service-a ──Bearer <service-token>──> service-b
     �? service-b 验签 + 检�?aud + 检�?scope
```

---

## 3. 用户身份层（JWT 透传 + 下游解析�?

### 3.1 api-gateway �?

**现有逻辑不变**�?
- RSA 离线验签（`services/api-gateway/src/jwt.rs`�?
- Redis 黑名单查�?

**关键变更**：**不再注入 X-User-* headers**，仅透传 Authorization header。

**Authorization 头原样透传**（下游服务必须从此解析用户身份）�?

### 3.2 下游微服务（Rust）�?

**依赖库**：`libs/jwt-ctx`（Rust 共享库）

**解析方式**：**decode payload only，不做密码学验签**。原因：
- 用户明确要求"下游不验�?jwt 是否合法"
- 验签意味着每个服务加载 RSA 公钥 + 每次请求 CPU 开销
- 安全前提：下游只监听内网，Authorization 必来�?api-gateway

**使用方式**：

```rust
// 使用 jwt-ctx 库的 Principal extractor
use jwt_ctx::Principal;

async fn get_current_user(me: Principal) -> Json<UserResponse> {
    Json(UserResponse {
        id: me.user_id,
        code: me.user_code,
        dept_id: me.dept_id,
        roles: me.roles,
    })
}
```

**jwt-ctx 库实现**：

```rust
// libs/jwt-ctx/src/principal.rs
impl Principal {
    pub fn from_headers(h: &HeaderMap) -> Result<Self, PrincipalError> {
        // 1. 从 Authorization header 提取 token
        // 2. 解码 JWT payload（base64 URL-safe + JSON 解析）
        // 3. 提取 claims：sub, user_code, dept_id, roles, permissions
    }
}
```

### 3.3 安全前提（部署规范）

- 下游服务**只监听内�?*，nginx / docker network / k8s NetworkPolicy 兜底
- api-gateway �?Authorization header 唯一来源
- 一旦下游误暴露公网，攻击者即可伪造身份——靠**网络�?+ 配置审计**兜底�?*�?*�?JWT 验签
- **禁止**下游服务读取或依赖 X-User-* headers

---

## 4. 下游业务授权

### 4.1 原则

- 授权不进通用中间�?
- 每个 handler 内部按业务规则判�?
- 提供可复用的 `authz` 辅助函数�?

### 4.2 典型规则（合同服务示例）

```go
// libs/go-authz/contract.go
func CanReadContract(user *Principal, c *Contract) bool {
  if c.OwnerID == user.UserID { return true }                              // owner
  if user.HasPermission("contract.read.all") { return true }               // 超管
  if user.IsDeptLeader && c.DeptID == user.DepartmentID { return true }    // 本部门主�?
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

### 4.4 一致�?

- 角色-权限关系�?user-service 维护
- 登录时一次写�?JWT，下游不�?DB
- 权限变更要等下次签发 JWT 才生效（接受，最终一致性）
- JWT 默认 TTL �?token-server 配置（参�?`2026-06-10-auth-login-security`�?

---

## 5. 服务对服务信任（client_credentials + service-token�?

### 5.1 适用场景

未来 service-a 需要直接调 service-b 内部接口（不�?api-gateway 转发给用户，或虽经网关但 service-b 需要识别调用方服务身份）�?

### 5.2 流程

```
service-a
  �?POST /api/tokens/access
  �?  grant_type=client_credentials
  �?  client_id=service-a
  �?  client_secret=***
  �?  scope=contract.read contract.write
  �?
token-server
  �?�?clients.json 校验 client_secret
  �?签发 service-token (RS256 JWT)
  �?claims: {
  �?  sub: "service-a",
  �?  scope: "contract.read contract.write",
  �?  aud: "contract-service",
  �?  token_type: "service",
  �?  exp: now + 1h
  �?}
  �?
service-a �?contract-service
  �?Authorization: Bearer <service-token>
  �?X-Service-Caller: service-a
  �?
contract-service
  �?�?RSA 验签（与用户 JWT 同密钥对�?
  �?�?检�?aud == "contract-service"
  �?�?检�?token_type == "service"
  �?�?检�?scope �?本操作所需 scope
```

### 5.3 与用�?JWT 的关键差�?

| 维度 | 用户 JWT | service-token |
|---|---|---|
| 验签 | 网关验，下游不验 | **下游必须验签** |
| `aud` | 可�?| 必填，限定目标服�?|
| `scope` | 标识 grant 类型 | 限定允许的操�?|
| `token_type` claim | 不存�?| `"service"` |
| TTL | 短（15-60min�?| 中（1h�?|
| 缓存 | 客户端持�?| service 本地缓存 |

### 5.4 客户端注�?

新增微服务必须先�?token-server 注册，否则拿不到 service-token�?

`docker-example/config/token-server/clients.json`�?
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

token-server 在签发时**主动校验**�?
- 请求 scope 必须�?`allowed_scopes` 子集
- 不允许为 service-a 签发 `aud=user-service` �?token

### 5.5 service-token 缓存

- service 本地缓存（含过期�?60s 余量），避免每请求换 token
- 缓存键：`{client_id} -> {token, exp}`
- 进程�?LRU 即可�?*�?*�?Redis

### 5.6 下游验证中间�?

```go
// 服务侧验�?service-token
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

**claim schema �?user-service 决定，token-server 不做假设**。token-server 只负责：注入标准声明（`sub` / `iss` / `aud` / `exp` / `iat` / `jti`�? 透传 user-service 给的自定�?claims�?

| claim | 来源 | 必填 | 说明 |
|---|---|---|---|
| `sub` | token-server | �?| 用户 ID（数字字符串，由调用方传入） |
| `iss` | token-server | �?| `"superpowers-token-server"` |
| `aud` | token-server | | 可�?|
| `exp` | token-server | �?| 过期时间�?|
| `iat` | token-server | �?| 签发时间�?|
| `jti` | token-server | �?| UUID |
| `client_id` | token-server | �?| 签发方客户端 ID |
| `scope` | token-server | �?| `"access"` / `"refresh"` / `"resource"` |
| `permissions` | user-service | �?| 细粒度权限点 |
| **`dept_id`** | user-service | ✓（access�?| 部门 ID |
| **`is_dept_leader`** | user-service | ✓（access�?| 是否部门主管 |
| **`roles`** | user-service | ✓（access�?| 粗粒度角�?|
| **其他业务字段** | user-service | | user-service 决定 schema，加新字段无需�?token-server |

**关键设计**�?
- `issue_access_token` 签名改为接受 `extra_claims: serde_json::Value`（或 `HashMap<String, Value>`），原样合并�?JWT custom claims
- 登录时调用链：`auth-service` �?`user-service.GetClaims(user_id, client_id)` �?返回完整 claims 字典 �?`token-server` 签名
- user-service 暴露内部端点 `GET /internal/users/{id}/claims`（仅内网�?*�?*�?api-gateway），�?auth-service / token-server 在签 token 前调�?
- 下游 Go 服务解析 claims 时不假设 schema，用通用 `Principal` 结构 + 业务字段按需读取
- user-service 是用户身份的真实源，加新字段只改 user-service + 下游消费方，不动 token-server

### 6.2 service-token

| claim | 类型 | 必填 | 说明 |
|---|---|---|---|
| `sub` | string | �?| 服务客户�?ID�?service-a"�?|
| `iss` | string | �?| token-server |
| `aud` | string | �?| 目标服务名（"contract-service"�?|
| `exp` | int64 | �?| |
| `iat` | int64 | �?| |
| `jti` | string | �?| UUID |
| `scope` | string | �?| 空格分隔的操作集�?|
| `token_type` | string | �?| 固定 `"service"` |

---

## 7. 错误处理

| 场景 | HTTP | error code |
|---|---|---|
| �?Authorization | 401 | `missing_token` |
| JWT base64 解析失败 | 401 | `invalid_token` |
| exp 已过 | 401 | `token_expired` |
| 业务规则拒绝 | 403 | `forbidden` |
| service-token 验签失败 | 401 | `invalid_token` |
| service-token aud 不匹�?| 403 | `wrong_audience` |
| service-token scope 不足 | 403 | `insufficient_scope` |
| service-token 类型�?| 401 | `wrong_token_type` |

**下游不查黑名�?*（黑名单由网关查），�?*解析 exp** 判断过期�?

---

## 8. 测试

### 8.1 单元

- `libs/jwt-ctx/`：JWT payload decode 成功/失败/过期/缺字段/无效格式
- `libs/authz/`：owner / 同部�?/ 部门主管 / 超管 / 默认拒绝矩阵

### 8.2 集成

- 验证 `jwt-ctx` 库与 axum extractor 集成
- 覆盖：缺失 Authorization 401 / 无效格式 401 / 正常解析 200

### 8.3 E2E

- �?4 服务，签发三种身份的 JWT
- 模拟外部用户访问接口
- 验证下游服务正确从 JWT payload 解析用户身份

---

## 9. 待办 / 风险

1. **claims 字段命名**：本设计已定 `dept_id` / `is_dept_leader` / `roles`，需 user-service 配合在登录时提供这些信息
2. **网络隔离规范**：需�?docker-compose / 部署文档明确下游服务端口不暴露公�?
3. **token-server 修改影响�?*：`issue_access_token` 签名变更，需同步更新现有集成测试
4. **service-token 引入�?*：先实现用户身份层，服务对服务等出现实际需求时再做

---

## 10. 改动清单（实现阶段）

| 路径 | 改动 |
|---|---|
| `libs/jwt-ctx/src/principal.rs` | 重写 Principal 提取逻辑：从 Authorization header 解析 JWT payload（不验签），提取 sub, user_code, dept_id, roles, permissions |
| `libs/jwt-ctx/Cargo.toml` | 添加 base64, serde, serde_json 依赖 |
| `libs/jwt-ctx/tests/` | 更新测试用例，覆盖 JWT 解析场景 |
| `services/notification-service/src/auth.rs` | 更新 Identity 提取逻辑，从 JWT payload 解析 |
| `services/notification-service/Cargo.toml` | 添加 base64 依赖 |
| `services/api-gateway/src/middleware/mod.rs` | **移除** X-User-* headers 注入逻辑（可选，保留不影响） |
| `docs/guides/microservice-auth.md` (新建) | 部署规范：端口隔离、网关唯一入口、JWT 解析规范 |
