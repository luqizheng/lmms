# 测试指南

> 面向工程师与 AI 协作。覆盖「加测试 / 改测试 / 跑测试 / AI 修测试」全流程。

---

## 0. 分层一览

| 层 | 跑在哪 | 入口 | 落点 |
|---|---|---|---|
| E2E-API | Docker 全栈（服务 + 网关） | `tests/e2e/api/cases/*.xlsx` | vitest + supertest（数据驱动） |
| E2E-UI | Docker 全栈 + 前端容器 | `tests/e2e/ui/cases/*.xlsx` | Playwright（数据驱动） |
| E2E（旧） | Docker 全栈（4 服务 + 网关） | `tests/e2e/*.e2e.test.ts` | vitest + supertest（手写） |
| 集成 | 单服务 + 真实 DB | `services/<svc>/tests/*_test.rs` | Rust integration test |
| 单元 | 进程内 | `services/<svc>/src/**` + `#[cfg(test)]`<br>`libs/*/src/**` + `#[cfg(test)]` | Rust unit test |
| Spec | 文档 / AI 提示词来源 | `tests/specs/**/*.yaml` | YAML |

e2e 走 API Gateway（端口 3000）。E2E-UI 经前端容器（8080）→ 网关。集成/单元走服务进程内或独立 DB。

> E2E 已重构为两部分（见 [§8 数据驱动 E2E](#8-数据驱动-e2e两部分)）：
> 第一部分直连网关验证 API；第二部分经前端做业务测试。旧 `*.e2e.test.ts` 仍保留并行。

---

## 1. 增加测试

### 1.1 加一个 E2E 用例

最小骨架：

```ts
// tests/e2e/<area>.e2e.test.ts
import { describe, it, expect } from 'vitest';
import request from 'supertest';
import { baseURL } from './helpers/http';
import { SEED } from './fixtures/seed';

describe('area', () => {
  it('should do X', async () => {
    const res = await request(baseURL).get('/api/v1/xxx');
    expect(res.status).toBe(200);
  });
});
```

要点：

- **认证**：用 `authedReq(adminToken).get/post(...)`，`adminToken` 来自 `globalSetup.ts` 注入的 `globalThis.__adminToken__`。
- **无认证**：`request(baseURL).get(...)` 直发。
- **不写死 user/role 名字**：用 `SEED.newUser()` / `Payload.register()` 这类工厂函数生成唯一名。
- **追踪清理**：创建的 user/role 调用 `trackUser(id)` / `trackRole(id)`，teardown 会清。
- **断言**：避免 `[200, 201]` 这种宽容匹配，落到具体状态码 + body 字段。

命名：用 `should <行为>` 句式，单 case 名可被 `vitest -t "<pattern>"` 命中。

### 1.2 加一个 Service 集成测试

```rust
// services/<svc>/tests/<feature>.rs
use sqlx::MySqlPool;

#[sqlx::test]
async fn create_user_inserts_row(pool: MySqlPool) {
    let req = /* ... */;
    let id = svc::create(&pool, &req).await.unwrap();
    assert!(id > 0);
}
```

要点：

- 用 `#[sqlx::test]` 自动建临时库（无需手写 setup）。
- 不连真实网络：mock 掉 `reqwest` / `redis` / `consul`。
- 每个 `it` 独立数据：通过 `uuid` / `nanoid` 生成唯一键，不依赖清理顺序。

### 1.3 加一个 Unit 测试

写在源码文件内 `#[cfg(test)] mod tests`：

```rust
pub fn is_valid_username(s: &str) -> bool { /* ... */ }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short() {
        assert!(!is_valid_username("ab"));
    }
}
```

适用于纯函数 / 校验逻辑 / 序列化。

### 1.4 加一个 Spec（AI codegen 入口）

```yaml
# tests/specs/<service>/<name>.yaml
name: login_with_valid_credentials
service: user-service
type: integration
description: 有效凭证登录返回 access_token
tags: [smoke, auth]
preconditions:
  - 数据库存在测试用户 test_user / Passw0rd!
steps:
  - method: POST
    path: /auth/login
    body:
      username: test_user
      password: Passw0rd!
expected:
  status: 200
  body_contains: access_token
data_cleanup:
  - DELETE FROM users WHERE username='test_user'
```

字段必填：`name` / `service` / `type` / `description` / `steps`。
`name` 必须匹配 `^[a-z][a-z0-9_]*$`，`service` ∈ 服务清单。

---

## 2. 更新测试

| 场景 | 操作 |
|---|---|
| 业务接口改了字段 | 同步更新对应 e2e / 集成测试的断言 |
| 路由前缀改了 | 更新 e2e 路径 + 检查 `docker/config/api-gateway/routes/*.router.toml` |
| 新增角色 / 权限 | 在 `users.e2e.test.ts` 的「assign and remove」链追加 |
| payload 改了 | 更新 `tests/e2e/fixtures/payload.ts` 和 `tests/e2e/fixtures/seed.ts` |

修改守则：

- 改 `routes/*.router.toml` 后**必须**先 `validate` 再 commit。
- 改 e2e 后必须能在**真实 docker 全栈**跑通，不允许 mock 网关绕过认证。
- 改集成测试后**必须**用 `cargo test -p <svc> --test <name>` 单测验证。

---

## 3. 执行测试

### 3.1 E2E（推荐方式：脚本一键）

```powershell
# 起栈（infrastructure/docker-compose.yml）
docker compose -f infrastructure/docker-compose.yml up -d

# 装依赖 + 跑全量 e2e
pnpm -C tests/e2e install
node scripts/run-e2e-local.js

# 只跑指定文件
node scripts/run-e2e-local.js users

# 只跑指定 case（透传 vitest 参数）
node scripts/run-e2e-local.js auth -t "register"
```

`run-e2e-local.js` 在 `tests/e2e` 目录下用 `vitest run` 执行，参数原样透传。

### 3.2 E2E（手动：栈已起）

```powershell
$env:E2E_BASE_URL = "http://localhost:3000"
$env:E2E_DEBUG = "1"
pnpm -C tests/e2e test                          # 全量
pnpm -C tests/e2e test auth                     # 单文件
pnpm -C tests/e2e test users -t "list users"    # 单 case
```

### 3.3 Rust 集成 / 单元

```powershell
# ⚠️ 必须在真实 PowerShell（vcvarsall x64）跑，沙箱 IDE 终端 cargo 会 LNK2019
# 沙箱内只允许只读命令：Get-Content / Select-String / git status 等

# 全量
cargo test --workspace

# 单服务
cargo test -p user-service

# 单文件
cargo test -p user-service --test integration_test

# 单用例
cargo test -p user-service integration_test::create_user

# 守门
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

### 3.4 Rust 单元测试（Docker）

```powershell
# 全部服务
node scripts/run-tests-docker.js

# 指定服务
node scripts/run-tests-docker.js iam-service token-service
```

容器内挂载源码跑 `cargo test`，详见根 `AGENTS.md`。

---

## 4. AI 修复测试

### 4.1 触发条件

- e2e 跑出 FAIL（`tests/e2e/shared/report/report-<时间戳>.xlsx` summary fail > 0）
- 或 `tests/e2e/shared/report/errors/<caseId>.json` 存在失败明细
- 或用户说「修测试」

### 4.2 流程

```powershell
# 1. 看失败明细
Get-Content tests\e2e\shared\report\errors\<caseId>.json

# 2. 把失败 request/response/expect 贴给 AI 分析

# 3. AI 改完后重跑验证
node scripts/run-e2e-local.js <area>
```

### 4.3 强约束（AI 必须遵守）

**修改边界**

| 允许改 | 禁止改 |
|---|---|
| `services/<svc>/tests/*.rs` | `services/<svc>/src/**`（业务源码） |
| `tests/e2e/*.e2e.test.ts` | `libs/*/src/**`（业务源码） |
| `tests/e2e/fixtures/**` | `sdk/**` 业务代码 |
| `tests/e2e/helpers/**` | `scripts/lib/*`（基础设施） |
| `tests/e2e/api/cases/**` / `ui/cases/**` | `infrastructure/config/api-gateway/routes/*.router.toml`（路由配置） |
| 报告 / 临时调试代码 | `Cargo.toml` 依赖（除非用户授权） |

> 触发修测试时，AI 默认**只改 tests/** 下代码。如果失败源于业务代码，先停下来跟用户确认。

**修复策略优先级**

1. **测试数据过期**：用户名/角色名/UUID 变了 → 同步测试 fixture
2. **断言过严**：服务端返回了合法但测试没覆盖的格式 → 放宽断言
3. **环境问题**：DB 状态污染 / 端口冲突 / token 过期 → 重置或调整 setup
4. **真实业务 bug**：测试正确，代码错了 → **停下来**，报告用户，不要改业务代码
5. **真要改业务代码**：必须用户明确授权，并补集成测试覆盖

---

## 5. 守门与 CI

提交前（按改动路径）：

| 改动 | 命令 |
|---|---|
| `services/*/src/**.rs` | `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test -p <svc>` |
| `services/*/tests/**.rs` | `cargo test -p <svc>` |
| `docker/config/api-gateway/routes/*.router.toml` | `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes` |
| `tests/e2e/**` | `node scripts/run-e2e-local.js`（栈已起时） |
| `sdk/java/**/pom.xml` | `mvn verify`（仅 SDK 仓库） |
| `sdk/dotnet/**/*.csproj` | `dotnet test`（仅 SDK 仓库） |
| `infrastructure/docker-compose.*.yaml` | 走 `coder-microservice-dev` skill 验证启动 |

CI tag 触发 SDK 发版（`sdk:publish`），本地**不要**手动 push。

---

## 6. 速查

| 想做 | 命令 |
|---|---|
| 起全栈 | `docker compose -f infrastructure/docker-compose.yml up -d` |
| 跑 e2e 全量 | `node scripts/run-e2e-local.js` |
| 跑 e2e 单 case | `node scripts/run-e2e-local.js <area> -t "<pattern>"` |
| 跑 rust 全量 | `cargo test --workspace` |
| 跑 rust 单服务 | `cargo test -p <svc>` |
| 跑 rust 守门 | `cargo fmt --check && cargo clippy --all-targets -- -D warnings` |
| 看失败明细 | `Get-Content tests\e2e\shared\report\errors\<caseId>.json` |
| 校验路由 | `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes` |
| 关栈 | `docker compose -f infrastructure/docker-compose.yml down` |

---

## 7. 反模式

- ❌ 在测试里写 `sleep(2000)` 等异步副作用
- ❌ 用 `[200, 201]` 模糊断言掩盖问题
- ❌ 直接改业务代码绕过测试失败
- ❌ 把 `private.pem` / `.env` / `node_modules` 写进测试 fixture
- ❌ 跨 case 共享可变状态（每个 `it` 独立 setup/teardown）
- ❌ 测试依赖外部网络（必须用本地 docker 栈）
- ❌ 沙箱 IDE 终端跑 `cargo`（必 LNK2019，去真实 PowerShell）

---

## 8. 数据驱动 E2E（两部分）

E2E 重构为两部分，共享底层设施（`tests/e2e/shared/`）。用例与报告均用 Excel 管理，失败输出 JSON 便于排查。

### 8.1 目录结构

```
tests/e2e/
  shared/
    config.ts               # 集中环境变量（baseURL/frontendURL/admin/db）
    db/
      reset.ts              # resetBase() / resetBusiness()
      seed/base.sql         # 基础数据（admin 由 API 注册，预留非用户基础数据扩展）
    data/excel.ts           # 读用例 xlsx + 占位符解析 + 断言
    report/
      writer.ts             # 写 report-<时间戳>.xlsx + 失败 <caseId>.json
      errors/               # 失败明细（json；UI 附 .png 截图）
  api/                      # 第一部分：直连 api-gateway
    cases/*.xlsx            # 数据驱动用例
    globalSetup.ts          # resetBase + 注册并登录 admin
    runner/run.e2e.test.ts  # 遍历 xlsx 执行
    vitest.config.ts
  ui/                       # 第二部分：经前端做业务测试
    cases/*.xlsx
    pages/                  # Page Object（login / user）
    flows/index.ts          # 业务流编排（flow 名 → 函数）
    globalSetup.ts          # resetBase + 注册 admin + 前端健康探测
    runner/run.spec.ts
    playwright.config.ts
  scripts/gen-sample-cases.mjs   # 生成样例 xlsx
```

### 8.2 数据分层

| 层 | 函数 | 时机 | 行为 |
|---|---|---|---|
| 基础数据 | `resetBase()` | 整轮开始（globalSetup） | 清全部表 + 注册 admin（幂等） |
| 业务数据 | `resetBusiness()` | 每套件前（beforeAll） | 只清业务表，保留 admin |

- 基础数据：admin 由 API `register` 创建（保证密码哈希与服务一致），非用户基础数据写 `seed/base.sql`（`INSERT ... ON DUPLICATE KEY UPDATE` 幂等）。
- 业务数据：业务表整表 `DELETE`；`users` 表按 `config.seedUsernames` 保留（`users.id` 为 UUID，不能按数值上限区分）。

### 8.3 Excel 用例格式

每个服务/业务流一个 `.xlsx`，每 sheet 一组，每行一个用例。

API 用例列（`api/cases/*.xlsx`）：

| 列 | 说明 |
|---|---|
| `caseId` | 用例唯一 ID，失败 json 以此命名 |
| `desc` | 描述 |
| `method` | GET/POST/PUT/PATCH/DELETE |
| `path` | 网关入口路径，如 `/v1/iam/users` |
| `headers` | 预留附加头（可空） |
| `body` | JSON 字符串，支持占位符 |
| `expectStatus` | 期望状态码 |
| `expectBody` | 断言表达式（见 8.4） |
| `skip` | `Y` 跳过 |

UI 用例列（`ui/cases/*.xlsx`）：

| 列 | 说明 |
|---|---|
| `caseId` / `desc` / `skip` | 同上 |
| `flow` | 在 `ui/flows/index.ts` 注册的编排名 |
| `params` | JSON 字符串，支持占位符 |
| `expect` | 当前支持 `toast=<文案>` 校验 |

### 8.4 占位符与断言

- 占位符：
  - `${rand}` → 唯一值
  - `${CASE-ID.field}` → 引用前序用例响应字段（同一执行上下文共享）
- 断言（`expectBody`）：
  - `$.x exists` → 字段存在
  - `$.x==v` → 字段等值
  - 多条用 `&&` 连接

### 8.5 报告与失败明细

- 报告：`shared/report/report-<时间戳>.xlsx`，含 `detail`（每用例 PASS/FAIL/SKIP、状态码、耗时）+ `summary`（total/pass/fail/skip）。
- 失败明细：`shared/report/errors/<caseId>.json`，含 request/response/expect/timestamp；UI 失败额外附 `<caseId>.png` 截图。
- 报告与 errors 已在 `.gitignore` 忽略，不入库。

### 8.6 运行

```powershell
pnpm -C tests/e2e test:api      # 第一部分：API 数据驱动
pnpm -C tests/e2e test:ui       # 第二部分：前端业务（需前端容器）
pnpm -C tests/e2e gen:samples   # 重新生成样例用例
```

前置：

- 基础设施 + 服务：`docker compose -f infrastructure/docker-compose.yml up -d`
- UI 测试要求前端容器**禁用登录验证码**：`infrastructure/docker-compose.yml` 已为 frontend 构建传入 `VITE_DISABLE_CAPTCHA=true`，改前端后需 `docker compose build frontend`。
- 首次跑 UI 需安装浏览器：`pnpm -C tests/e2e exec playwright install chromium`（配置使用 `channel: "chromium"` 完整构建）。

### 8.7 新增用例

1. 编辑或新增 `api/cases/*.xlsx` / `ui/cases/*.xlsx`（列见 8.3）。
2. UI 用例的 `flow` 字段需在 `ui/flows/index.ts` 注册对应编排（用 `pages/` 下 Page Object 操作前端）。
3. 直接重跑 `test:api` / `test:ui`，无需改执行器代码。

### 8.8 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `E2E_BASE_URL` | `http://localhost:3000` | API Gateway |
| `E2E_FRONTEND_URL` | `http://localhost:8080` | 前端容器 |
| `E2E_ADMIN_USER` | `e2eadmin` | admin 用户名 |
| `E2E_ADMIN_PASS` | `P@ssw0rd!` | admin 密码 |
| `E2E_DB_*` | 见 `shared/config.ts` | host/port/user/password/database |
| `E2E_SEED_USERNAMES` | `e2eadmin` | 业务数据清理时保留的用户名（逗号分隔） |
| `E2E_DEBUG` | `0` | `1` 输出调试信息 |
