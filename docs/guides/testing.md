# 测试指南

> 面向工程师与 AI 协作。覆盖「加测试 / 改测试 / 跑测试 / AI 修测试」全流程。

---

## 0. 分层一览

| 层 | 跑在哪 | 入口 | 落点 |
|---|---|---|---|
| E2E | Docker 全栈（4 服务 + 网关） | `tests/e2e/*.e2e.test.ts` | vitest + supertest |
| 集成 | 单服务 + 真实 DB | `services/<svc>/tests/*_test.rs` | Rust integration test |
| 单元 | 进程内 | `services/<svc>/src/**` + `#[cfg(test)]`<br>`libs/*/src/**` + `#[cfg(test)]` | Rust unit test |
| Spec | 文档 / AI 提示词来源 | `tests/specs/**/*.yaml` | YAML |

e2e 走 API Gateway（端口 3000）。集成/单元走服务进程内或独立 DB。

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
| YAML spec 改字段 | 重跑 `validate-specs` 确认通过 |

修改守则：

- 改 `routes/*.router.toml` 后**必须**先 `validate` 再 commit。
- 改 e2e 后必须能在**真实 docker 全栈**跑通，不允许 mock 网关绕过认证。
- 改集成测试后**必须**用 `cargo test -p <svc> --test <name>` 单测验证。

---

## 3. 执行测试

### 3.1 E2E（推荐方式：脚本一键）

```powershell
# 一键：起栈 + 装依赖 + 跑测
node scripts/run-e2e.js

# 只跑指定文件
node scripts/run-e2e.js --filter users.e2e.test.ts

# 只跑指定 case
node scripts/run-e2e.js --filter auth.e2e.test.ts --test-name "register"

# 跳过 docker up（栈已起）
node scripts/run-e2e.js --no-up

# 强制重建镜像
node scripts/run-e2e.js --rebuild
```

等价于：`docker compose -f infrastructure/docker-compose.yml up -d` → 等网关 200 → `pnpm -C tests/e2e test`。

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

### 3.4 Orchestrator 闭环（test-orchestrator）

```powershell
node scripts/test-orchestrator.js start      # 起基础设施
node scripts/test-orchestrator.js reset-db    # 重建测试库
node scripts/test-orchestrator.js test-all    # 全量 Rust 测试
node scripts/test-orchestrator.js test-integration --service user-service
node scripts/test-orchestrator.js report      # 生成 reports\run-*\summary.md
node scripts/test-orchestrator.js loop        # start + reset-db + test-all + report
node scripts/test-orchestrator.js stop        # 关栈
```

### 3.5 Spec 校验

```powershell
node scripts/test-orchestrator.js validate-specs
node scripts/test-orchestrator.js generate-prompt   # 输出 reports\spec-to-code-prompt.md
```

---

## 4. AI 修复测试

### 4.1 触发条件

- `reports/run-*/summary.md` 出现 `fail > 0`
- 或 `reports/latest-fix-prompt.md` 存在且 mtime < 1h
- 或用户说「修测试 / fix-prompt」

### 4.2 流程

```powershell
# 1. 看失败摘要
Get-Content reports\latest-fix-prompt.md

# 2. 把 fix-prompt 内容贴给 AI
#    含三段：## 失败用例 / ## 错误详情 / ## 相关文件

# 3. AI 改完后，验证
node scripts/test-orchestrator.js reset-db
node scripts/test-orchestrator.js test-integration
```

### 4.3 强约束（AI 必须遵守）

**修改边界**

| 允许改 | 禁止改 |
|---|---|
| `services/<svc>/tests/*.rs` | `services/<svc>/src/**`（业务源码） |
| `tests/e2e/*.e2e.test.ts` | `libs/*/src/**`（业务源码） |
| `tests/e2e/fixtures/**` | `sdk/**` 业务代码 |
| `tests/e2e/helpers/**` | `scripts/lib/*`（基础设施） |
| `tests/specs/**/*.yaml` | `docker/config/api-gateway/routes/*.router.toml`（路由配置） |
| 报告 / 临时调试代码 | `Cargo.toml` 依赖（除非用户授权） |

> 触发 `coder-fix-tests` 时，AI 默认**只改 tests/** 下代码。如果失败源于业务代码，先停下来跟用户确认。

**修复策略优先级**

1. **测试数据过期**：用户名/角色名/UUID 变了 → 同步测试 fixture
2. **断言过严**：服务端返回了合法但测试没覆盖的格式 → 放宽断言
3. **环境问题**：DB 状态污染 / 端口冲突 / token 过期 → 重置或调整 setup
4. **真实业务 bug**：测试正确，代码错了 → **停下来**，报告用户，不要改业务代码
5. **真要改业务代码**：必须用户明确授权，并补集成测试覆盖

**重生成 fix-prompt**

```powershell
node scripts/test-orchestrator.js fix-prompt
# 模板在 scripts\prompts\fix-tests.template.md
```

---

## 5. 守门与 CI

提交前（按改动路径）：

| 改动 | 命令 |
|---|---|
| `services/*/src/**.rs` | `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test -p <svc>` |
| `services/*/tests/**.rs` | `cargo test -p <svc>` |
| `docker/config/api-gateway/routes/*.router.toml` | `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes` |
| `tests/specs/**.yaml` | `node scripts/test-orchestrator.js validate-specs` |
| `sdk/java/**/pom.xml` | `mvn verify`（仅 SDK 仓库） |
| `sdk/dotnet/**/*.csproj` | `dotnet test`（仅 SDK 仓库） |
| `infrastructure/docker-compose.*.yaml` | 走 `coder-microservice-dev` skill 验证启动 |

CI tag 触发 SDK 发版（`sdk:publish`），本地**不要**手动 push。

---

## 6. 速查

| 想做 | 命令 |
|---|---|
| 起全栈 + 跑 e2e | `node scripts/run-e2e.js` |
| 跑 e2e 单 case | `node scripts/run-e2e.js --filter <file> --test-name "<pattern>"` |
| 跑 rust 全量 | `cargo test --workspace` |
| 跑 rust 单服务 | `cargo test -p <svc>` |
| 跑 rust 守门 | `cargo fmt --check && cargo clippy --all-targets -- -D warnings` |
| 重置测试库 | `node scripts/test-orchestrator.js reset-db` |
| 看最近报告 | `Get-Content reports\run-*\summary.md \| Select-Object -Last 1` |
| 校验路由 | `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes` |
| 校验 spec | `node scripts/test-orchestrator.js validate-specs` |
| 生成 fix-prompt | `node scripts/test-orchestrator.js fix-prompt` |
| 关栈 | `node scripts/test-orchestrator.js stop` |

---

## 7. 反模式

- ❌ 在测试里写 `sleep(2000)` 等异步副作用
- ❌ 用 `[200, 201]` 模糊断言掩盖问题
- ❌ 直接改业务代码绕过测试失败
- ❌ 把 `private.pem` / `.env` / `node_modules` 写进测试 fixture
- ❌ 跨 case 共享可变状态（每个 `it` 独立 setup/teardown）
- ❌ 测试依赖外部网络（必须用本地 docker 栈）
- ❌ 沙箱 IDE 终端跑 `cargo`（必 LNK2019，去真实 PowerShell）
