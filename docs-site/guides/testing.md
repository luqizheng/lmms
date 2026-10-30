# 测试指南（Testing Guide）

> 最近更新：2026-06-25

本文档涵盖：

1. 三层测试金字塔（unit → integration → e2e）
2. 本地与 CI 执行测试
3. 数据驱动 e2e 用例格式
4. 快速反馈的 smoke 子集
5. 启动前健康检查（preflight）

---

## 1. 测试金字塔

```
        ┌──────────────────────────────────────┐
   e2e  │  Vitest API（数据驱动）              │  全量 ~5 min / smoke ~2 min
        │  在 docker 中跑真实服务              │
        ├──────────────────────────────────────┤
  intg  │  Rust cargo test --test <name>       │  单服务 ~30s
        │  sqlx / mockall / testcontainers     │
        ├──────────────────────────────────────┤
  unit  │  Rust cargo test --lib               │  单服务 ~5s
        │  纯业务逻辑，无 I/O                   │
        └──────────────────────────────────────┘
```

| 层级 | 单服务 / 全量耗时 | 适用场景 |
|------|-------------------|----------|
| Unit | ~5s | 纯逻辑，无 DB / 网络 |
| Integration | ~30s | DB / 网络 / testcontainers |
| E2E (smoke) | ~2 min | 关键 happy path |
| E2E (API 全量) | ~5 min | 所有 API 契约 |

---

## 2. 执行测试

### 2.1 单元 + 集成（按服务）

```bash
# 跑某个服务的全部测试（基于 docker，无需本机 cc linker）
node scripts/run-tests-docker.js iam-service

# 多服务
node scripts/run-tests-docker.js iam-service token-service notification-service

# 全部服务
node scripts/run-tests-docker.js
```

等价的底层 docker 命令：

```bash
docker run --rm --network host \
  -v $(pwd):/app \
  -v coder-cargo-registry:/usr/local/cargo/registry \
  -v coder-cargo-git:/usr/local/cargo/git \
  -v coder-cargo-target:/app/target \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -w /app \
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl \
  cargo test --target x86_64-unknown-linux-musl -p iam-service -- --nocapture
```

### 2.2 按名字跑单个测试

```bash
# 集成测试
cargo test --test integration_test -- --nocapture test_login

# 单元测试
cargo test --lib -- --nocapture create_user
```

### 2.3 E2E 测试

```bash
cd tests/e2e
pnpm install  # 仅首次

# Smoke 子集（快，~2 min）
pnpm test:smoke           # API smoke

# 全量 API
pnpm test:api
```

> E2E 只有数据驱动 API 测试，直接打 api-gateway（`E2E_BASE_URL`）。原 Playwright UI 套件（`test:ui` / `test:smoke:ui`）已随 PRISM console 移除删除。

### 2.4 环境变量

| 变量 | 默认 | 用途 |
|------|------|------|
| `E2E_BASE_URL` | `http://localhost:3000` | api-gateway URL |
| `E2E_ADMIN_USER` | `e2eadmin` | 自动注册的管理员账号 |
| `E2E_ADMIN_PASS` | `P@ssw0rd!` | 管理员密码 |
| `E2E_DEBUG` | `0` | 设为 `1` 输出请求 / 响应详细日志 |
| `E2E_SMOKE` | 未设置 | 设为 `1` 过滤到 smoke 子集 |
| `E2E_RETRY` | `1` | 覆盖 vitest 重试次数 |

---

## 3. 数据驱动 E2E 用例格式

所有 e2e 用例写在 YAML 里，按服务或业务流程一个文件。

### 3.1 API 用例格式

`tests/e2e/api/cases/<service>.yaml`：

```yaml
group: <service-group-name>
cases:
  - caseId: <PREFIX>-<NUMBER>
    desc: <人类可读描述>
    smoke: true                       # 加入 smoke 子集（可选）
    method: POST | GET | PUT | DELETE | PATCH
    path: /v1/iam/users
    auth: false                       # 默认 true；false 表示匿名请求
    headers:                          # 额外 header（可选）
      X-Trace-Id: <32-hex>
    body:                             # 请求 body（任意 JSON）
      username: new${rand}            # ${rand} 生成唯一值
      password: P@ssw0rd!
    expectStatus: 200                 # 或写成 [200, 201] 表示容忍
    expect:                           # body 断言（可选）
      - $.id exists                   # JSONPath + 操作符 + 期望值
      - $.username==new${rand}
    timeoutMs: 30000                  # 覆盖默认 20s
    skip: false                       # 标记为跳过
```

### 3.2 断言表达式

`expect` 支持 5 种操作符：

| 表达式 | 含义 |
|--------|------|
| `$.field exists` | 字段存在且非空 |
| `$.field not exists` | 字段缺失或为空 |
| `$.field is array` | 字段是数组 |
| `$.field contains <substr>` | 字段（字符串化后）包含子串 |
| `$.field==<value>` | 字段等于指定值（按字符串比较） |

### 3.3 占位符

| 占位符 | 替换为 |
|--------|--------|
| `${rand}` | 随机值 `Date.now() + 4-char hex` |
| `${case-id.field}` | 引用前一个 case 响应中的某字段 |

示例：`${IAM-USER-001.id}` 引用了 `IAM-USER-001` 返回的 `id`。

---

## 4. Smoke 子集

### 4.1 什么是 smoke

smoke 是一批被 `smoke: true` 标记的关键路径用例，用途是开发时拿到快速反馈。

| 层级 | 时长 | 用例数 | 适用时机 |
|------|------|--------|----------|
| **smoke** | ~2 min | 关键路径 API 用例 | 提交前、迭代开发 |
| **api** | ~5 min | 50+ | 发布前、CI |

### 4.2 执行 smoke

```bash
cd tests/e2e

# 通过环境变量
E2E_SMOKE=1 pnpm test:api

# 或者用专门脚本
pnpm test:smoke
```

### 4.3 把某用例加入 smoke

在 YAML 用例里设置 `smoke: true`：

```yaml
cases:
  - caseId: NOTIF-001
    desc: 创建公告
    smoke: true       # ← 加入 smoke
    method: POST
    ...
```

### 4.4 什么算好的 smoke 用例

- 某个端点的关键 happy path
- 不依赖前置用例（或依赖极少）
- 执行快（无大文件上传、无长等待）
- 验证的是真实生产场景

应当标 smoke：

- `AUTH-001`（登录）
- `GW-001`（网关路由）
- `HEALTH`（健康检查）
- `IAM-USER-001`（创建用户）
- 每个服务的一条关键路径

不要标 smoke：

- 依赖大量前置用例的
- 边界 / 错误路径用例
- 批量操作

---

## 5. 启动前健康检查（preflight）

任何 e2e 跑之前会先做 preflight，确认基础设施是好的，防止因基础设施挂掉导致一连串假阳性测试失败。

### 5.1 检查项

`tests/e2e/shared/health.ts` 会探测：

| 探针 | URL | 期望 |
|------|-----|------|
| `api-gateway /health` | `${E2E_BASE_URL}/health` | 200 |
| `iam via gateway` | `${E2E_BASE_URL}/iam/health` | 200 |

每个探针 3 秒超时。

### 5.2 失败后

```
preflight api-gateway /health FAILED: ...
  → 请先启动 infrastructure: docker compose -f infrastructure/docker-compose.yml up -d
```

修复：先把基础设施跑起来：

```bash
docker compose -f infrastructure/docker-compose.yml up -d
```

然后再跑测试。

### 5.3 为什么要 preflight

没有 preflight 的话，基础设施挂掉会导致 50+ 用例失败，但每条报错都形似业务测试 bug，定位很费时。有了 preflight，一条清晰的错误就能指向根因。

---

## 6. 重试策略

### 6.1 Vitest

默认重试 1 次（失败后再跑一次）。用 `E2E_RETRY=N` 覆盖：

```bash
E2E_RETRY=0 pnpm test:api      # 不重试（CI 更严）
E2E_RETRY=3 pnpm test:smoke    # 多重试（网络抖动）
```

### 6.2 什么时候重试有益、什么时候是在掩盖 bug

重试能解决：

- 偶发网络问题（MySQL pool reset）
- 时序敏感的测试（WS 消息到达）

重试会掩盖：

- 生产代码的竞态（如 etag 冲突 —— 应当明确失败而不是重试）
- 测试数据准备不完整

如果某用例长期需要重试才能通过，**这是个臭味**，应当查根因而不是加重试。

---

## 7. 报告

### 7.1 Excel 报告

每次 e2e 跑完会生成 Excel 报告：

```
tests/e2e/shared/report/report-<timestamp>.xlsx
```

Sheet：

- **明细**：每条用例的结果、状态、耗时、错误信息
- **汇总**：总数 / 通过 / 失败 / 跳过 计数

### 7.2 失败详情

对每条失败用例还会写一份 JSON：

```
tests/e2e/shared/report/errors/<caseId>.json
```

内容：

```json
{
  "caseId": "IAM-USER-002",
  "request": { "method": "GET", "path": "...", "headers": {}, "body": null },
  "response": { "status": 500, "body": {} },
  "expect": { "status": 200, "body": "$.username==..." },
  "timestamp": "..."
}
```

UI 失败还会同时保存截图：

```
tests/e2e/shared/report/errors/<caseId>.png
```

### 7.3 查看报告

```bash
# macOS
open tests/e2e/shared/report/report-*.xlsx

# Linux
xdg-open tests/e2e/shared/report/report-*.xlsx

# 程序读取：
python3 -c "import openpyxl; wb = openpyxl.load_workbook('tests/e2e/shared/report/report-latest.xlsx'); print(wb.sheetnames)"
```

---

## 8. CI 集成

### 8.1 GitLab CI Job

`.gitlab-ci.yml` 中定义了：

| Job | Stage | 用途 |
|-----|-------|------|
| `rust_test` | test | `cargo test --workspace --lib` |
| `e2e_api_test` | e2e | `pnpm -C tests/e2e run test:api`（走 `E2E_BASE_URL` = staging api-gateway） |
| `service_standard_check` | test | 服务规范门禁 |
| `spec_check` | test | OpenAPI / 错误码 / 权限码门禁 |
| `contracts_check` | test | 契约门禁 |

### 8.2 Pre-commit 钩子

通过 `.pre-commit-config.yaml`（pre-commit 框架）或 `scripts/git-hooks/pre-commit`（零依赖兜底）配置。

钩子在每次 commit 前会跑 service-standard / spec / contracts / openapi-drift 门禁。E2E **不**会在 pre-commit 里跑（太慢），本地快速反馈用 `pnpm test:smoke`。

---

## 9. 常见模式

### 9.1 跨用例复用上下文

```yaml
# 第一个用例创建用户
- caseId: USER-CREATE
  method: POST
  path: /v1/iam/users
  body:
    username: alice-${rand}
  expectStatus: 200
  expect:
    - $.id exists

# 第二个用例引用前一个用例的 id
- caseId: USER-GET
  method: GET
  path: /v1/iam/users/${USER-CREATE.id}  # 引用前置用例
  expectStatus: 200
  expect:
    - $.username matches alice-*
```

### 9.2 容忍多个状态码

```yaml
expectStatus: [200, 201]   # 200 或 201 都接受
```

适用场景：

- 服务可能返回 200，也可能返回 201
- 新版返回 200，旧版返回 201（迁移期间）

### 9.3 测多步业务流程

API 用例通过 `${case-id.field}` 引用前置用例的响应字段，可以串出多步业务流程（原 UI flow 用例已随 PRISM console 移除删除）：

```yaml
# 创建 + 查询 + 验证，一条链
- caseId: NOTIF-CREATE
  method: POST
  path: /v1/notifications
  params:
    title: e2e-${rand}
  expect:
    - $.id exists

- caseId: NOTIF-GET
  method: GET
  path: /v1/notifications/${NOTIF-CREATE.id}
  expectStatus: 200
```

---

## 10. 调试

### 10.1 打开详细日志

```bash
E2E_DEBUG=1 pnpm test:api
```

会把每个请求 / 响应输出到 stderr。

### 10.2 跑单个用例

```bash
# Vitest
cd tests/e2e
pnpm vitest run --config api/vitest.config.ts -t "AUTH-001"
```

### 10.3 排查测试状态

```bash
# 看 runner 输出
node scripts/run-tests-docker.js iam-service 2>&1 | tail -50

# 看基础设施健康
docker ps
docker logs <container>
curl http://localhost:3000/health
```

### 10.4 常见问题

| 现象 | 处理 |
|------|------|
| 测试“本地通过、CI 失败” | 检查环境变量；CI 可能 MySQL / Redis 版本不同 |
| 一连串测试失败（50 条） | 多半是基础设施挂了；看 preflight 输出 |
| 测试“首次失败、重试通过” | 是竞态，应查根因，而不是加重试次数 |

---

_本指南随代码库一起维护。设计思路见 `docs/superpowers/specs/2026-06-25-e2e-stability-design.md`。_
