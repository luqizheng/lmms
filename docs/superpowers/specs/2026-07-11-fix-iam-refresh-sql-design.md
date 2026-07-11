# Fix: iam refresh 端点 SQL 漏字段 → refresh 永远 TokenExpired（Bug #1）

> 日期：2026-07-11
> 关联：docs/superpowers/specs/2026-07-11-e2e-coverage-and-bugs.md §2 Bug #1
> 范围：`services/iam-service/src/service/auth_svc.rs::refresh` 单函数修复
> 风险：低（单 SQL 字段补齐，行为可观测，有 e2e 验证）

---

## 1. 现状

### 1.1 Bug 行为

`AuthService::refresh`（services/iam-service/src/service/auth_svc.rs:167-185）查询 refresh token 时只 SELECT 6 字段：

```rust
let token: Option<RefreshToken> = sqlx::query_as(
    "SELECT id, user_id, token_hash, expires_at, revoked, created_at FROM iam_refresh_tokens WHERE token_hash = ?"
)
.bind(&token_hash)
.fetch_optional(&self.state.db)
.await?;
```

但 `RefreshToken` struct 有 10 字段：

```rust
// services/iam-service/src/models/refresh_token.rs
pub struct RefreshToken {
    pub id: String,                    // ✓
    pub user_id: String,               // ✓
    pub token_hash: String,            // ✓
    pub expires_at: DateTime<Utc>,     // ✓
    pub client_ip: String,             // ✗ MISSING
    pub user_agent: Option<String>,     // ✗ MISSING
    pub revoked: bool,                 // ✓
    pub revoked_at: Option<DateTime<Utc>>, // ✗ MISSING
    pub created_at: DateTime<Utc>,     // ✓
    pub updated_at: DateTime<Utc>,     // ✗ MISSING
}
```

### 1.2 根因链

1. `sqlx::query_as` + `FromRow` derive：缺少的 4 字段用 `Default::default()` 填充
2. `DateTime<Utc>::default()` = `DateTime::from_timestamp(0, 0).unwrap()` = **1970-01-01 00:00:00 UTC**
3. `if token.expires_at < Utc::now()` 永远为真（1970 < 2026）
4. 返回 `IamError::TokenExpired` → 401

### 1.3 生产影响

- **任何 refresh_token 流程立即失败**——用户登录后 1 秒内刷新必报 401
- 不是"TTL 到了"，是**第一次刷新就 401**
- e2e `AUTH-REFRESH-001`（已在 auth-lifecycle.yaml 中，标记 skip）一直 FAIL，揭示了这个 bug
- 任何走 OAuth refresh 流程的 SDK 都受影响

### 1.4 对比：同文件已有正确 SELECT

line 265（list_sessions）已有正确 10 字段 SQL：

```rust
"SELECT id, user_id, token_hash, expires_at, client_ip, user_agent, revoked, revoked_at, created_at, updated_at FROM iam_refresh_tokens WHERE user_id = ? AND revoked = FALSE ORDER BY created_at DESC"
```

line 171 的 SELECT 字段少 4 个。

### 1.5 已有发现途径

- `tests/e2e/api/cases/auth-lifecycle.yaml` — `AUTH-REFRESH-001` 标 skip + desc 注明揭示 bug（commit d5331a1）
- 修复后将 un-skip 此 case 实际跑通

---

## 2. 修复方案

### 2.1 选择：补齐 SELECT 字段（最小修改）

把 line 171 SQL 改为 10 字段，与 line 265 格式一致：

```rust
let token: Option<RefreshToken> = sqlx::query_as(
    "SELECT id, user_id, token_hash, expires_at, client_ip, user_agent, revoked, revoked_at, created_at, updated_at FROM iam_refresh_tokens WHERE token_hash = ?"
)
.bind(&token_hash)
.fetch_optional(&self.state.db)
.await?;
```

字段顺序与 `RefreshToken` struct 严格对应（sqlx `FromRow` derive 按声明顺序填充）。

### 2.2 为什么不用其他方案

- **不重构 `RefreshToken` struct**：现有 10 字段是其他 query（list_sessions、revoke_session）正常工作的，bug 单纯是 line 171 的 SQL 字段缺失
- **不重写 refresh 逻辑**：当前 refresh 逻辑（查 token → 验证 → 查 user → 重发 token）本身正确，只需修 SQL

---

## 3. 验证步骤

### 3.1 编译 + 单测

```bash
cd infrastructure && docker compose -f docker-compose.yml build iam-service
# 单测（如果有）：
SWAGGER_UI_DOWNLOAD_URL=file://$(pwd)/infrastructure/vendor/swagger-ui-v5.17.14.zip \
  cargo test -p iam-service --lib
```

期望：编译通过，iam-service 单测全过。

### 3.2 e2e 验证（关键）

先 un-skip `AUTH-REFRESH-001`（之前标 skip 揭示 bug）：

```yaml
# tests/e2e/api/cases/auth-lifecycle.yaml
- caseId: AUTH-REFRESH-001
  desc: admin 用 refresh_token 换发新 token
  smoke: true  # 改为 true
  method: POST
  path: /v1/iam/auth/refresh
  auth: false
  body:
    refresh_token: ${AUTH-REFRESH-LOGIN.refresh_token}
  expectStatus: 200
  expect:
    - $.access_token exists
    - $.refresh_token exists
    - $.token_type==Bearer
  # 移除: skip: true
```

跑 e2e：
```bash
cd tests/e2e && docker run --rm --network infrastructure_superpowers \
  -v $(pwd):/app -w /app -e E2E_DB_HOST=mysql -e E2E_BASE_URL=http://api-gateway:3000 \
  zhcoder-docker-registry.com:8000/official/node:20 \
  npx vitest run --config api/vitest.config.ts -t 'AUTH-REFRESH'
```

**期望**：
- `AUTH-REFRESH-LOGIN`：200 + `$.refresh_token exists`
- `AUTH-REFRESH-001`：**200**（修复前 401） + `$.access_token exists`

### 3.3 直接 curl 复现验证

```bash
# 1. admin login 拿 refresh_token
LOGIN=$(curl -s -X POST http://localhost:3000/v1/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"e2eadmin","password":"P@ssw0rd!"}')
RT=$(echo "$LOGIN" | python3 -c "import json,sys;print(json.load(sys.stdin)['refresh_token'])")

# 2. 立即 refresh
curl -s -w "\nHTTP %{http_code}\n" -X POST http://localhost:3000/v1/iam/auth/refresh \
  -H 'Content-Type: application/json' \
  -d "{\"refresh_token\":\"$RT\"}"
```

**修复前**：`HTTP 401`，body=`token expired`
**修复后**：`HTTP 200`，body 含新 `access_token` + `refresh_token`

### 3.4 iam-service 日志验证

```bash
docker logs infrastructure-iam-service-1 --tail 50 | grep -E "refresh|TokenExpired"
```

**修复后**：无 `TokenExpired` 错误日志（除用户真实 token 过期场景）

---

## 4. 风险评估

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| 字段顺序与 struct 不匹配 | 低 | sqlx 解析失败，编译错 | 已验证 line 265 的 10 字段顺序与 struct 一致，本次改动同模式 |
| 编译失败 | 低 | CI 阻断 | 改动只动 SQL 字符串，编译能立即发现 |
| 现有 refresh 用户（如果有）受影响 | 极低 | 改进了正确性 | 之前所有 refresh 调 401，本次修后正常 200 |

---

## 5. 不在范围内

- 重构 `RefreshToken` struct 字段（已有 query 正常工作）
- 修 `iam-auth-logout` 不联动 gateway 黑名单（Bug #2，独立 PR）
- 修 audit export 路径参数（Bug #3，独立 PR）
- 加 refresh token 轮换（security 增强，单独设计）

---

## 6. 验证完成标准（Definition of Done）

- [ ] iam-service 编译通过
- [ ] cargo test -p iam-service 全过
- [ ] e2e `AUTH-REFRESH-001` exit 0（un-skip 后）
- [ ] 直接 curl 复现：refresh 返回 200 + 新 token
- [ ] docs/superpowers/specs/2026-07-11-e2e-coverage-and-bugs.md 标 Bug #1 已修
- [ ] 大报告 §5 建议下一步：Bug #1 改为已修