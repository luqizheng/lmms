# Fix: iam logout 不联动 gateway 黑名单 → access_token TTL 内仍可用（Bug #2）

> 日期：2026-07-11
> 关联：docs/superpowers/specs/2026-07-11-e2e-coverage-and-bugs.md §2 Bug #2
> 范围：`libs/jwt-ctx/src/principal.rs`（+jti 字段）+ `services/iam-service/src/{routes,service}/auth.{rs,svc}`（logout 写 blacklist）
> 风险：中（共享 lib 改动 + Redis 写入路径新增）

---

## 1. 现状

### 1.1 Bug 行为

`iam /iam/auth/logout` 只撤销 refresh_token（DB `iam_refresh_tokens.revoked = true`），**不通知 gateway 把 access_token 的 jti 加 Redis 黑名单**。

结果：access_token 在 TTL 内仍能通过 gateway `auth_middleware` 的 `is_blacklisted` 检查（key 不存在 → 返回 false → pass），用户"登出"实际无效。

### 1.2 根因链

1. 用户调 `POST /v1/iam/auth/logout`（带 `Authorization: Bearer <access_token>`）
2. iam `routes/auth.rs::logout` 调 `AuthService::logout(&principal.user_id)`
3. `service/auth_svc.rs::logout` 只 `UPDATE iam_refresh_tokens SET revoked = true`（line 243-244）
4. **没有写 Redis blacklist** ← bug 位置
5. Gateway 后续请求的 `auth_middleware` line 167 `is_blacklisted(...)` 返回 false（key 不存在）
6. access_token 在 TTL（默认 15 min）内持续可用

### 1.3 Gateway 端已有检查（无需修改）

`services/api-gateway/src/blacklist.rs:13`：
```rust
let key = format!("{}blacklist:{}", prefix, jti);
conn.exists(&key).await
```

`services/api-gateway/src/middleware/mod.rs:167-172`：
```rust
let blacklisted = crate::blacklist::is_blacklisted(&mut conn, &state.redis_prefix, &claims.jti).await.unwrap_or(false);
if blacklisted {
    return (StatusCode::UNAUTHORIZED, "Token revoked").into_response();
}
```

Gateway 检查机制已就绪，**只需 iam 写 key**。

### 1.4 生产影响

- 用户"登出"实际只清服务端 refresh_token 状态
- access_token 在 TTL（15 min）内仍能被持有者使用直到过期
- 安全预期被打破（合规/审计问题）
- 跨服务调用链：用户 logout 后 token 仍可调其他 service 的端点

### 1.5 已有发现途径

- `tests/e2e/api/cases/auth-lifecycle.yaml` — `AUTH-LIFE-007` 标 skip + desc 注明揭示 bug
- 修复后将 un-skip 此 case 实际跑通

---

## 2. 修复方案

### 2.1 三处改动

| 文件 | 改动 |
|---|---|
| `libs/jwt-ctx/src/principal.rs` | `Principal` struct 新增 `jti: String` 字段；`from_claims` 从 JWT payload 提取 |
| `services/iam-service/src/routes/auth.rs::logout` | 把 `principal.jti` 传给 `AuthService::logout` |
| `services/iam-service/src/service/auth_svc.rs::logout` | 接收 `jti` 参数，写 Redis blacklist key |

### 2.2 改动 1：Principal 加 jti

```rust
// libs/jwt-ctx/src/principal.rs
pub struct Principal {
    pub jti: String,                    // ← 新增
    pub user_id: String,
    pub user_code: String,
    // ... 其余不变
}

// from_claims 内
let jti = payload
    .get("jti")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();

// 构造 Self { ... } 时加 jti 字段
```

**为什么必须改 Principal**：
- 备选方案 1：logout handler 重新 decode JWT 拿 jti — 重复工作 + 重新验签
- 备选方案 2：把 jti 塞 request extension — 增加复杂度
- 备选方案 3：Principal 加 jti — 干净、自然（jti 是 JWT 标准 claim）

**影响面**：
- `Principal` 是 `Debug, Clone` struct，无 `Default`（测试用 `from_claims` 构造）
- 现有测试 0 处用 struct literal `Principal { ... }`（grep 验证），全通过 `from_claims` / `from_token` 构造
- 改动向后兼容：现有调用方只读 `principal.user_id` 等，不受影响

### 2.3 改动 2：AuthService::logout 写 Redis blacklist

```rust
// services/iam-service/src/service/auth_svc.rs
pub async fn logout(&self, user_id: &str, jti: &str) -> Result<(), IamError> {
    // 1. 撤销所有 refresh tokens（原有逻辑）
    sqlx::query(
        "UPDATE iam_refresh_tokens SET revoked = TRUE, revoked_at = NOW() WHERE user_id = ? AND revoked = FALSE"
    )
    .bind(user_id)
    .execute(&self.state.db).await?;

    // 2. 写 Redis blacklist（新增）
    if let Some(conn) = self.state.redis.as_ref() {
        let mut conn = conn.clone();
        let key = format!("{}blacklist:{}", self.state.redis_prefix, jti);
        let ttl_sec = self.state.config.jwt.access_token_ttl_minutes * 60;
        let _: () = conn.set_ex(&key, "1", ttl_sec as u64).await?;
    }
    // Redis 不可用时 fail open（与 perm_epoch 同模式）
    
    Ok(())
}
```

**TTL 选择**：
- 用 access_token 完整 TTL（`config.jwt.access_token_ttl_minutes * 60`）
- 简化逻辑：blacklist key 与 token 寿命同步自动过期
- 不需要精确计算 `exp - now`（精度无收益）

**Fail open**：
- `self.state.redis` 是 `Option<ConnectionManager>`（生产环境必填，测试可为 None）
- Redis 不可用时跳过 blacklist 写入（fail open）
- 与 `stamp_perm_epochs` 模式一致（gateway consumer 兜底，see AGENTS.md §"令牌失效 perm_epoch"）

### 2.4 改动 3：logout handler 传 jti

```rust
// services/iam-service/src/routes/auth.rs::logout
pub async fn logout(
    State(state): State<AppState>,
    principal: Principal,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let service = AuthService::new(state);
    service.logout(&principal.user_id, &principal.jti).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(StatusCode::NO_CONTENT)
}
```

### 2.5 不在范围内

- `revoke_session`（按 session_id 单 session 撤销）— 该函数接收 `session_id`（refresh token id），不是 access token jti，机制不同，单独设计
- 多设备 session 一次性全部撤销（需 user-level epoch，单独设计）
- 改 fail-open 语义 — 设计层面议题
- 改 gateway 端 blacklist 检查 — 已就绪，无需改

---

## 3. 验证步骤

### 3.1 编译

```bash
cd infrastructure && docker compose -f docker-compose.yml build iam-service
```

期望：编译通过（Principal 加字段，调用方 0 处破坏）。

### 3.2 cargo test

```bash
SWAGGER_UI_DOWNLOAD_URL=file://$(pwd)/infrastructure/vendor/swagger-ui-v5.17.14.zip \
  cargo test -p iam-service --lib
# 也跑 jwt-ctx 测试：
cargo test -p jwt-ctx
```

期望：所有单测全过（Principal 现有测试通过 `from_claims` 构造，自动带 jti 字段）。

### 3.3 e2e 验证（关键）

先 un-skip `AUTH-LIFE-007`：

```yaml
# tests/e2e/api/cases/auth-lifecycle.yaml
- caseId: AUTH-LIFE-007
  desc: 登出后临时 token 调 sessions — 验证 blacklist 生效（修复后应 401）
  method: GET
  path: /v1/iam/auth/sessions
  headers:
    Authorization: Bearer ${AUTH-LIFE-002.access_token}
  expectStatus: 401
  # 移除: skip: true
```

跑 e2e：
```bash
cd tests/e2e && docker run --rm --network infrastructure_superpowers \
  -v $(pwd):/app -w /app -e E2E_DB_HOST=mysql -e E2E_BASE_URL=http://api-gateway:3000 \
  zhcoder-docker-registry.com:8000/official/node:20 \
  npx vitest run --config api/vitest.config.ts -t 'AUTH-LIFE-006|AUTH-LIFE-007'
```

**期望**：
- `AUTH-LIFE-006`（logout）：204
- `AUTH-LIFE-007`（用 logout 后 token 调 sessions）：**401**（修复前 200）

### 3.4 直接 redis-cli 验证

```bash
# 1. admin login 拿 token + jti
LOGIN=$(curl -s -X POST http://localhost:3000/v1/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"e2eadmin","password":"P@ssw0rd!"}')
TOKEN=$(echo "$LOGIN" | python3 -c "import json,sys;print(json.load(sys.stdin)['access_token'])")
JTI=$(echo "$TOKEN" | cut -d. -f2 | base64 -d | python3 -c "import json,sys;print(json.load(sys.stdin)['jti'])")
echo "JTI=$JTI"

# 2. logout
curl -s -w "\nHTTP %{http_code}\n" -X POST http://localhost:3000/v1/iam/auth/logout \
  -H "Authorization: Bearer $TOKEN"

# 3. 检查 Redis blacklist key
docker exec infrastructure-redis-1 redis-cli -n 0 EXISTS "superpowers:blacklist:$JTI"
# 期望: 1 (key 存在)

# 4. 验证 token 已失效
curl -s -w "\nHTTP %{http_code}\n" -X GET http://localhost:3000/v1/iam/auth/sessions \
  -H "Authorization: Bearer $TOKEN"
# 期望: 401 (Token revoked)
```

---

## 4. 风险评估

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| Principal 加 jti 字段破坏现有调用 | 低 | 编译失败 | 现有测试 0 处用 struct literal 构造，全走 `from_claims` |
| Redis 写入失败导致 logout 失败 | 低 | 用户登出失败 | Fail open（与 perm_epoch 同模式），DB refresh token 仍撤销 |
| TTL 设置过短导致 blacklist 提前过期 | 低 | token 过期前能复用 | 用 access_token 完整 TTL（15 min），与 token 寿命同步 |
| Gateway 检查与 iam 写入时序 | 极低 | 竞态 | Redis 单线程，写入后再返回 204，客户端收到 204 后才有后续请求 |

---

## 5. 不在范围内

- `revoke_session` 单 session 撤销（需 session_id → jti 映射，单独设计）
- 多设备 session 一次性全撤销（需 user-level logout_epoch）
- 改 fail-open 语义

---

## 6. 验证完成标准（Definition of Done）

- [ ] `cargo build -p iam-service` 通过
- [ ] `cargo test -p iam-service --lib` 全过
- [ ] `cargo test -p jwt-ctx` 全过
- [ ] e2e `AUTH-LIFE-006` 204 + `AUTH-LIFE-007` 401（un-skip 后）
- [ ] 直接 redis-cli 验证 blacklist key 存在
- [ ] 直接 curl 复现：logout 后 token 返回 401
- [ ] 大报告标 Bug #2 已修