# 微服务身份认证规则

## 1. 身份传递规范

### 1.1 强制规则

**禁止使用 X-User-* headers**：下游微服务**必须**从 `Authorization: Bearer <jwt>` header 解析用户身份，**不得**依赖或读取 `X-User-Id`、`X-User-Roles`、`X-User-Dept` 等 X-User-* headers。

### 1.2 推荐做法

- 使用 `libs/jwt-ctx` 库的 `Principal` extractor 提取用户身份
- 从 JWT payload 中提取以下 claims：
  - `sub`: 用户 ID（必填）
  - `user_code`: 用户编码（可选）
  - `dept_id`: 部门 ID（可选）
  - `roles`: 角色列表（JSON array）
  - `permissions`: 权限列表（JSON array）

### 1.3 安全原则

- 下游服务**不验证** JWT 签名，仅解码 payload
- 信任前提：api-gateway 已完成 RSA 验签和黑名单检查
- 下游服务只监听内网端口，禁止直接暴露公网

## 2. 代码规范

### 2.1 Rust 服务

```rust
// 正确做法：使用 jwt-ctx 库
use digua_jwt_ctx::Principal;

async fn handler(me: Principal, State(state): State<AppState>) -> impl IntoResponse {
    let user_id = me.user_id;
    let has_role = me.has_role("admin");
    // ...
}
```

### 2.2 禁止做法

```rust
// 错误：直接读取 X-User-* headers
// 此方式已废弃，禁止使用
let user_id = headers.get("x-user-id");
```

## 3. 网关行为

- api-gateway **不再注入** X-User-* headers（或仅作为调试用途）
- Authorization header **原样透传**到下游服务
- 网关负责：RSA 验签、Redis 黑名单检查、exp 过期检查

## 4. 违规处理

- 代码审查时发现使用 X-User-* headers 应拒绝合并
- 现有代码应逐步迁移到 JWT 解析方式
- 新增服务必须遵循此规则

## 5. 参考文档

- `docs/superpowers/specs/2026-06-10-microservice-jwt-verification-design.md`
- `libs/jwt-ctx/src/principal.rs`