---
name: coder-openapi-contracts-sync
description: |
  End-to-end post-edit validation workflow for the Rust microservices monorepo.
  Chains: modify routes → regenerate openapi.yaml → cross-check contracts → run 5 CI gates.
  Use after AI finishes adding/editing routes or services and before committing or pushing.
  Explicit trigger phrases: "openapi drift", "contracts check", "sync contracts",
  "改完路由了", "跑门禁", "post-edit validation", "before commit", "验证契约".
---

# coder-openapi-contracts-sync Skill

路由修改后的端到端验证工作流。AI 改完任何路由或服务代码之后、提交之前，必须完整跑完本流程。

## 何时用

AI 修改了以下内容之一时调用本 skill：
- 新增或修改了 `services/<svc>/src/routes/*.rs` 中的路由 handler
- 新增或修改了 `#[utoipa::path]` 注解
- 新增或修改了 `services/<svc>/src/openapi.rs` 中的 path 或 schema 注册
- 新增或修改了 `contracts/error-codes.yaml`、`contracts/permissions.yaml`、`contracts/events.yaml`
- 新增或修改了 `services/<svc>/migrations/**/*.sql` 中的表名
- 用户明确要求："跑门禁"、"check gates"、"before commit"、"验证契约"

## 前置

- 运行环境：Windows / macOS / Linux（Windows 走 host 原生 MSVC，脚本自动注入 VS 环境变量）
- 工作目录：项目根目录
- Node.js 18+
- Rust 工具链（用于 `cargo run --bin dump_openapi`）

## 工作流

### 步骤 0：确认改动范围

在开始之前，先确认本次改动的范围，选择性跳过后续不相关步骤：

| 改动类型 | 跳过步骤 |
|---|---|
| 仅改 Rust 路由，未动 contracts YAML | 步骤 3、4（contracts-gen + check-spec 中的权限码/错误码部分）|
| 仅改 contracts YAML，未动路由 | 步骤 1、2（仅跑 contracts-gen + check-spec）|
| 仅改 migrations 表名 | 仅跑步骤 5（check-db-naming）|

### 步骤 1：重新生成 openapi.yaml

```bash
node scripts/dump-openapi-specs.js
```

- 脚本对 `services/` 下每个服务执行 `cargo run --bin dump_openapi`（Rust 服务）或检查 `openapi.yaml` 存在（Java 服务）
- `cargo run` 的工作目录是 `services/<svc>`，产物写入 `services/<svc>/openapi.yaml`
- 若 `cargo run` 失败（如 target 目录权限问题），脚本会 warn 但不 exit error；committed 的 openapi.yaml 会被保留
- **exit code 0** 表示全部服务生成成功；非 0 表示有服务生成失败

### 步骤 2：检查 openapi drift

```bash
git diff --exit-code -- 'services/*/openapi.yaml'
```

- 若有 drift（文件内容发生变化），exit code 为 1；无 drift 为 0
- 有 drift 时：**必须**将 openapi.yaml 改动与路由改动放在同一 commit 中提交

### 步骤 3：重新生成契约代码

仅当改动了 `contracts/error-codes.yaml`、`contracts/permissions.yaml` 或 `contracts/events.yaml` 时执行：

```bash
node scripts/contracts-gen.js
```

生成产物：
- `libs/digua_common-error/src/codes.rs`（错误码常量）
- `libs/digua_authz/src/perms.rs`（权限码常量）
- `libs/digua_common_event/src/events.rs`（事件 struct）
- `frontend/src/contracts/*.ts`（TypeScript 类型，若前端已就绪）

**禁止**：编辑 `contracts/*.yaml` 后不运行 contracts-gen 就提交；生成代码与 YAML 不一致是 CI 红的首要原因。

### 步骤 4：运行 check-spec

```bash
node scripts/check-spec.js
```

检查范围：
- `contracts/permissions.yaml`：格式 `SERVICE_RESOURCE_ACTION`，无重复
- `contracts/error-codes.yaml`：格式 `SERVICE_CATEGORY_NUMBER`，无重复
- `services/*/openapi.yaml`：必填字段（operationId、summary）、paths 完整性

**exit code 1** 表示有规范违反；**exit code 0** 表示全部通过。

### 步骤 5：运行 check-db-naming

```bash
node scripts/check-db-naming.js
```

检查范围：
- `services/*/migrations/**/*.sql` 中所有 `CREATE TABLE` / `ALTER TABLE` 的表名必须以有效前缀开头
- `services/*/src/**/*.rs` 中 sqlx 字符串里的表名必须以有效前缀开头

有效前缀：`iam_`、`audit_`、`token_`、`ntf_`、`cfg_`、`file_`、`prue_`。

**exit code 0** = PASS；**exit code 1** = FAIL（有表名缺少前缀）。

### 步骤 6：运行 check-service-standard

```bash
node scripts/check-service-standard.js
```

检查全部 8 项服务规范（每个服务的 AGENTS.md 中声明豁免的项目除外）：

| 检查项 | 说明 |
|--------|------|
| Dockerfile | 服务必须有 Dockerfile |
| AGENTS.md | 服务必须有 AGENTS.md |
| README.md | 服务必须有 README.md |
| tests/ 目录 | 必须有测试文件 |
| /health 端点 | 必须有路由 |
| /metrics 端点 | 必须有路由 |
| migrations/ 目录 | 必须有 migrations 目录 |
| Swagger 配置 | 必须有 SWAGGER_ENABLED 开关 |

**exit code 1** 表示有服务不合规；**exit code 0** 表示全部通过。

## 5 个门禁脚本对照表

| # | 脚本 | 用途 | 检查内容 | exit 0 | exit 1 常见原因 |
|---|------|------|----------|--------|----------------|
| 1 | `node scripts/check-service-standard.js` | 服务规范审计 | 8 项规范（Dockerfile / AGENTS.md / README / tests / health / metrics / migrations / swagger） | 全部合规 | 缺少必需文件或端点 |
| 2 | `node scripts/check-spec.js` | 契约规范检查 | error-codes.yaml 格式、permissions.yaml 格式、openapi.yaml 必填字段 | 全部通过 | 格式错误、缺少 operationId、重复码 |
| 3 | `node scripts/contracts-gen.js --check` | 契约生成一致性（CI 模式） | 生成的 Rust/TS 代码与 contracts YAML 必须完全匹配 | 代码与 YAML 一致 | YAML 被改但生成产物未更新 |
| 4 | `node scripts/check-db-naming.js` | 表名前缀检查 | SQL 和 sqlx 字符串中表名必须有 `iam_`/`audit_`/`token_`/`ntf_`/`cfg_`/`file_`/`prue_` 前缀 | 无违规 | 表名缺少前缀 |
| 5 | `git diff --exit-code -- 'services/*/openapi.yaml'` | OpenAPI drift 检测 | openapi.yaml 必须与 git 中 committed 版本一致（路由改动后已重新生成） | 无 drift | 路由改了但 openapi.yaml 未重新生成或未提交 |

> 注：步骤 3 的 `contracts-gen.js` 不带 `--check` 时仅生成代码，不做 drift 检查；CI 中必须用 `--check` 模式检测漂移。

## 常见失败与修复

### openapi.yaml drift 检测失败（git diff exit 1）

**原因**：路由改了但 openapi.yaml 未重新生成或未提交。
**修复**：
```bash
node scripts/dump-openapi-specs.js
git add services/*/openapi.yaml
# 与路由改动一起 commit
```

### contracts-gen drift 检测失败（exit 1）

**原因**：编辑了 `contracts/*.yaml` 但未运行 `contracts-gen.js`，或运行后未提交生成产物。
**修复**：
```bash
node scripts/contracts-gen.js
git add libs/digua_common-error/src/codes.rs libs/digua_authz/src/perms.rs
git commit -m "contracts: regenerate from YAML"
```

### check-spec 报告 format error

**原因**：`error-codes.yaml` 中码格式不符合 `SERVICE_CATEGORY_NUMBER`（如少下划线、数字位置错误），或 `permissions.yaml` 中码不符合 `SERVICE_RESOURCE_ACTION`。
**修复**：对照 `contracts/http-conventions.md` 修正格式后重新运行。

### check-db-naming 报告 bare table name

**原因**：在 SQL 字符串或 migrations 中使用了不带前缀的表名（如 `login_logs` 而非 `audit_login_logs`）。
**修复**：按 `contracts/db-conventions.md` 中表名前缀映射表修正。

### check-service-standard 报告 missing Dockerfile

**原因**：新建服务后未创建 Dockerfile。
**修复**：参照 `services/<已有服务>/Dockerfile` 创建新服务的 Dockerfile。

## 禁止事项

- **禁止**编辑 `contracts/*.yaml` 后不运行 `contracts-gen.js` 就提交。生成代码与 YAML 不一致时 CI 必定 red。
- **禁止**在路由改动后不运行 `dump-openapi-specs.js` 也不提交 openapi.yaml drift 就推送。CI 的 git diff 检测会失败。
- **禁止**在 CI 中单独运行 `contracts-gen.js` 而不带 `--check` 标志；必须用 `--check` 才能检测漂移。
- **禁止**将 openapi.yaml 改动和路由改动分开提交。两者必须在同一 commit 中，以保持历史可追溯性。
- **禁止**在 `check-spec.js` 失败时跳过（规范违反是 CI 红的直接原因）。

## 验证

提交之前，核查以下检查清单：

1. `node scripts/dump-openapi-specs.js` 执行完毕且 exit 0
2. `git diff --exit-code -- 'services/*/openapi.yaml'` exit 0（无 drift 或 drift 已 staged）
3. 若改动了 contracts YAML：`node scripts/contracts-gen.js` 执行完毕
4. `node scripts/check-spec.js` exit 0
5. `node scripts/check-db-naming.js` exit 0
6. `node scripts/check-service-standard.js` exit 0
7. 所有改动的文件已 `git add`（路由代码 + openapi.yaml + 生成的 Rust 文件）
8. Commit message 格式符合 `<Phase>:<summary>` 或 `feat(<scope>): <summary>`
