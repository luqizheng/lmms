# Fix: audit 合规导出路径参数不一致 → 500 (Bug #3)

> 日期：2026-07-11
> 关联：docs/superpowers/specs/2026-07-11-e2e-coverage-and-bugs.md §2 Bug #3
> 范围：`services/audit-service/src/routes/compliance.rs`（router）+ `openapi.yaml` + README
> 风险：低（单文件 router 改动 + 文档同步）

---

## 1. 现状

### 1.1 Bug 行为

`audit-service` 的 3 个合规导出端点都返回 500：

```bash
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/v1/audit/export/compliance/login/csv
# 实际：500 "Wrong number of path arguments for `Path`. Expected 1 but got 0"
```

### 1.2 根因

`services/audit-service/src/routes/compliance.rs:10-16`：
```rust
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/export/compliance/operation/csv", get(export_operation_csv))   // ← 0 path 参数
        .route("/export/compliance/operation/json", get(export_operation_json))
        .route("/export/compliance/login/csv", get(export_login_csv))
        .with_state(state)
}
```

但 3 个 handler 签名都要求 1 个 path 参数：
```rust
// compliance.rs:26-29
pub async fn export_operation_csv(
    State(state): State<AppState>,
    Path(user_id): Path<String>,  // ← 期望 1 个 path 参数
) -> AppResult<...> { ... }
```

axum 0.8 严格检查：router 注册 0 path 参数但 handler 声明 1 个 → 500 "Wrong number of path arguments"。

OpenAPI schema 也写 0 path 参数（与 router 一致），三方完全脱节只在于 handler 签名。

### 1.3 现有 service 方法

`services/audit-service/src/services/audit.rs`：
- `get_user_operation_logs(user_id, limit)` — 按 user_id 查 operation logs
- `get_user_login_logs(user_id, limit)` — 按 user_id 查 login logs
- 无 "get all" 方法

handler 逻辑本身合理（per-user 导出），bug 单纯是 router 少了 path 参数。

### 1.4 生产影响

- 合规导出（CSV / JSON）整个功能不可用
- 外部审计、合规留存受影响
- 业务侧调 `/v1/audit/export/compliance/{login,operation}/{csv,json}` 全 500

### 1.5 已有发现途径

- `tests/e2e/api/cases/audit.yaml` — `AUDIT-EXPORT-001/002/003` 标 skip + desc 注明揭示 bug
- `scripts/e2e-rate-limit.js`（无关）— e2e 全量回归时报出

---

## 2. 修复方案

### 2.1 选择：方案 A — router 加 `/{user_id}` 后缀

理由：
- handler 已有 `Path(user_id)` 接收参数，逻辑（`get_user_operation_logs(user_id, 10000)`）per-user 合理
- 无需新增 service 方法
- router / handler / openapi 三方统一
- API 语义清晰：每个用户的数据独立导出

### 2.2 改动 1：router 加 `/{user_id}`

```rust
// services/audit-service/src/routes/compliance.rs
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/export/compliance/operation/csv/{user_id}", get(export_operation_csv))
        .route("/export/compliance/operation/json/{user_id}", get(export_operation_json))
        .route("/export/compliance/login/csv/{user_id}", get(export_login_csv))
        .with_state(state)
}
```

### 2.3 改动 2：openapi.yaml 加 Path 参数

```yaml
/audit/export/compliance/login/csv/{user_id}:
  get:
    tags:
    - compliance
    operationId: export_login_csv
    parameters:
    - name: user_id
      in: path
      required: true
      schema:
        type: string
    responses: ...
```

类似 3 个端点都加。

### 2.4 改动 3：e2e AUDIT-EXPORT-* un-skip + 路径加 user_id

```yaml
# tests/e2e/api/cases/audit.yaml
- caseId: AUDIT-EXPORT-001
  desc: 合规导出登录日志 CSV
  method: GET
  path: /v1/audit/export/compliance/login/csv/${AUTH-001.id}  # ← 加 user_id
  expectStatus: 200
```

`AUTH-001` 是已有 case（admin login 返回 user.id）。但 `auth-001` 不在 audit.yaml — 需要跨文件引用。改用 admin 自己的 user_id（`${IAM-USER-001.id}` 或 hardcode `e2eadmin` 的 id）。

**更简单**：直接传 admin login 拿到的 user_id。改 path 用 admin id 即可。

### 2.5 改动 4：README 更新示例

```bash
# 修复前
curl http://localhost:8080/audit/export/operation/csv

# 修复后
curl http://localhost:8080/audit/export/operation/csv/u1
```

### 2.6 不选方案 B 的原因

- 方案 B：handler 改无 path 参数 + 加 query 参数过滤
- 问题：当前 service 只有 per-user 方法，无 "get all"。要新加 service 方法 + handler 改 body
- 改动面更大，且 per-user 语义更合理（合规导出按 user 隔离）

---

## 3. 验证步骤

### 3.1 编译

```bash
cd infrastructure && docker compose -f docker-compose.yml build audit-service
```

期望：编译通过。

### 3.2 e2e 验证（关键）

先 un-skip `AUDIT-EXPORT-001/002/003` + path 加 user_id：

```bash
# admin login 拿 user_id
TOKEN=$(curl ... | jq -r .access_token)
USER_ID=$(curl ... | jq -r .user.id)

# 测 3 个端点
curl -w "\nHTTP %{http_code}\n" "http://localhost:3000/v1/audit/export/compliance/login/csv/$USER_ID" \
  -H "Authorization: Bearer $TOKEN"
curl -w "\nHTTP %{http_code}\n" "http://localhost:3000/v1/audit/export/compliance/operation/csv/$USER_ID" \
  -H "Authorization: Bearer $TOKEN"
curl -w "\nHTTP %{http_code}\n" "http://localhost:3000/v1/audit/export/compliance/operation/json/$USER_ID" \
  -H "Authorization: Bearer $TOKEN"
```

**期望**：3 个端点都返回 200 + CSV/JSON 内容（修复前 500）。

### 3.3 e2e 全量跑

```bash
cd tests/e2e && docker run --rm --network infrastructure_superpowers \
  -v $(pwd):/app -w /app -e E2E_DB_HOST=mysql -e E2E_BASE_URL=http://api-gateway:3000 \
  zhcoder-docker-registry.com:8000/official/node:20 \
  npx vitest run --config api/vitest.config.ts
```

**期望**：`AUDIT-EXPORT-001/002/003` 3/3 pass（un-skip 后）

---

## 4. 风险评估

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| API breaking change（路径多了 user_id） | 中 | 旧调用方 404 | 文档同步（README + openapi），e2e 同步更新；目前生产路径 500，等于 0 实际调用方 |
| gateway 路由不匹配 | 低 | gateway 404 | gateway router 用 `{rest}` 通配，路径细节在 audit-service 内部 |
| 跨文件占位符引用 | 低 | e2e 跑失败 | 在 audit.yaml 自包含建临时用户 |

---

## 5. 不在范围内

- 改 fail-safe：handler 兜底无 user_id 返回 400
- 改 service 方法支持"全量"导出
- 改 audit 表结构

---

## 6. 验证完成标准（Definition of Done）

- [ ] `docker compose build audit-service` 通过
- [ ] e2e `AUDIT-EXPORT-001/002/003` 3/3 pass（un-skip 后）
- [ ] 直接 curl：3 个端点都返回 200（修复前 500）
- [ ] openapi.yaml 与 router 一致
- [ ] 大报告标 Bug #3 已修