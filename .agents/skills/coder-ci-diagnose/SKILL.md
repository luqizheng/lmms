---
name: coder-ci-diagnose
description: |
  Diagnose CI gate failures and map red error messages to root causes with fix commands.
  Use when the user says "CI failed", "lint failed", "openapi drift", "contracts 检查失败",
  "spec 不通过", "服务规范检查未通过", "红标", "CI 红", "check-service-standard 失败",
  "check-spec 失败", "check-db-naming 失败", "dump-openapi-specs 失败",
  or when AI proactively wants to know what to look out for before pushing.
---

# coder-ci-diagnose Skill

CI 失败翻译表：红 error 信息 → 根因分类 → 一键修复命令。

## 何时用

- CI pipeline 显示红标（failed），需要快速定位根因
- push 前想跑一遍本地门禁确认无事
- 用户说"CI 失败了"、"lint 没过"、"openapi drift"、"contracts 检查失败"
- 三类高频踩坑（占全部 CI 失败约 70%）：
  - 改了路由但忘记 `dump-openapi-specs`
  - 在 Rust 代码里加了错误码但没同步到 `contracts/error-codes.yaml`
  - SQL 里用了裸表名（无服务前缀）

## 5 道门禁失败对照表

### A. 服务规范类（`check-service-standard.js`）

触发命令：`node scripts/check-service-standard.js`

| 错误信息片段（原文） | 触发的检查项 | 根因分类 | 修复命令 | 预防建议 |
|---|---|---|---|---|
| `⚠️ 缺失: Dockerfile` | Dockerfile 存在性 | 新建服务未执行 coder-new-service | 参照 `services/<svc>/Dockerfile` 脚手架补全 | 用 `coder-new-service` skill 新建服务 |
| `⚠️ 缺失: AGENTS.md` | AGENTS.md 存在性 | 新建服务未生成规范文件 | 参照已有服务（如 `services/audit-service/AGENTS.md`）补全 | 同上 |
| `⚠️ 缺失: README.md` | README.md 存在性 | 新建服务脚手架不完整 | 参照 `services/token-service/README.md` 格式补全 | 同上 |
| `⚠️ 缺失: tests/ 目录` 或 `⚠️ 缺失: tests/ 目录` | tests/ 目录或测试文件 | 缺少单元测试文件 | 在 `services/<svc>/tests/` 下补充 `*_test.rs` | Rust 服务：`tests/` 目录必须有 `.rs` 文件 |
| `⚠️ 缺失: /health 端点` | /health 路由注册 | 路由文件未注册健康检查端点 | 在路由或 main.rs 中补充 `.route("/health", get(handler))` | 已有 `#[utoipa::path]` 时确认 main.rs 有注册 |
| `⚠️ 缺失: /metrics 端点` | /metrics 路由注册 | 缺少 metrics 端点 | 同上，注册 `/metrics` 路由（utoipa 不自动注册） | 同上 |
| `⚠️ 缺失: migrations/ 目录` | migrations/ 目录存在性 | 无数据库迁移目录 | 创建 `services/<svc>/migrations/` 目录并补充迁移文件 | 用 `coder-migrate-db` skill 加迁移 |
| `⚠️ 缺失: Swagger 配置（SWAGGER_ENABLED 开关）` | SWAGGER_ENABLED 存在性 | `Cargo.toml` 无 `utoipa-swagger-ui` 或 main.rs 无 `SWAGGER_ENABLED` | 确认 `Cargo.toml` 含 `utoipa-swagger-ui`，main.rs 含 `SWAGGER_ENABLED` 守卫 | 参照 `services/token-service/src/main.rs` 配置 |
| `❌ 服务规范检查未通过`（process.exit(1)） | 上述任意项未通过 | 任意 8-item 规范缺失 | 逐项补全后重新运行脚本 | push 前跑一遍本地检查 |

### B. OpenAPI 类（`check-spec.js` + `dump-openapi-specs.js`）

#### B1. OpenAPI 字段规范性（`check-spec.js`）

触发命令：`node scripts/check-spec.js`

| 错误信息片段（原文） | 触发的检查项 | 根因分类 | 修复命令 | 预防建议 |
|---|---|---|---|---|
| `路径 ${pathName} ${method} 缺少 operationId` | OpenAPI operationId 必填 | `#[utoipa::path]` 注解未写 `operationId` 参数 | 在路由 handler 的 `#[utoipa::path]` 中添加 `operation_id = "handler_name"` | 每个路由必须显式写 operation_id |
| `⚠️ 路径 ${pathName} ${method} 缺少 summary（建议添加）` | OpenAPI summary（warning，不阻断） | utoipa 默认不生成 summary | 在 `#[utoipa::path]` 中补充 `summary = "描述文字"` | 加 summary 有助于前端理解 API 用途 |
| `缺少 openapi 字段` | openapi 版本字段 | openapi.yaml 格式被破坏 | 重新运行 `node scripts/dump-openapi-specs.js` | 不要直接编辑 openapi.yaml |
| `缺少 info.title 或 info.version` | info 字段完整性 | 同上 | 同上 | 同上 |
| `缺少 paths 字段` | paths 字段存在性 | 同上 | 同上 | 同上 |
| `JSON 解析错误: ${e.message}` | YAML 格式损坏 | 同上 | 同上 | 同上 |
| `❌ 总发现 ${errors.length} 个问题`（process.exit(1)） | 任意上述字段缺失 | OpenAPI spec 不合规 | 确认 openapi.yaml 由 utoipa 生成，不要手改 | 同上 |

#### B2. OpenAPI Drift（`dump-openapi-specs.js` + `git diff`）

触发命令：`node scripts/dump-openapi-specs.js && git diff --exit-code -- 'services/*/openapi.yaml'`

| 错误信息片段（原文） | 触发的检查项 | 根因分类 | 修复命令 | 预防建议 |
|---|---|---|---|---|
| `git diff --exit-code` 返回非零（drift detected） | openapi.yaml 与 utoipa 产物不一致 | 改了路由/注解但忘记重新生成 openapi.yaml | `node scripts/dump-openapi-specs.js && git add -A && git commit -m "fix: regenerate openapi after route changes"` | 每次改 `#[utoipa::path]` 后立刻跑 dump-openapi-specs.js |
| `WARN: rust dump failed for ${svc} — keep committed openapi.yaml` | Rust dump 二进制执行失败 | utoipa dump 二进制编译失败或有错 | 先 `cargo check -p <svc>` 修复编译错误，再重新 dump | 改完代码先确保 cargo build 通过 |

### C. 契约类（`check-spec.js` + `contracts-gen.js --check`）

#### C1. 错误码格式与重复（`check-spec.js`）

| 错误信息片段（原文） | 触发的检查项 | 根因分类 | 修复命令 | 预防建议 |
|---|---|---|---|---|
| `错误码格式不正确: ${code}，应为 SERVICE_CATEGORY_NUMBER 格式` | 错误码格式（应为 `SERVICE_CATEGORY_NUMBER`） | YAML 中写了不符号格式的错误码 | 在 `contracts/error-codes.yaml` 中将 `${code}` 改为 `SERVICE_CATEGORY_NUMBER` 格式 | 格式必须是 `大写服务_大写类别_数字`，如 `IAM_USER_NOT_FOUND_1001` |
| `重复的错误码: ${code}` | 错误码唯一性 | YAML 中同一错误码出现多次 | 在 `contracts/error-codes.yaml` 中删除或合并重复项 | 不要在 YAML 中写重复条目 |

#### C2. 权限码格式与重复（`check-spec.js`）

| 错误信息片段（原文） | 触发的检查项 | 根因分类 | 修复命令 | 预防建议 |
|---|---|---|---|---|
| `权限码格式不正确: ${code}，应为 SERVICE_RESOURCE_ACTION 格式` | 权限码格式（应为 `SERVICE_RESOURCE_ACTION`） | YAML 中写了不符合格式的权限码 | 在 `contracts/permissions.yaml` 中将 `${code}` 改为 `SERVICE_RESOURCE_ACTION` 格式 | 格式必须是 `大写服务_大写资源_大写动作`，如 `IAM_USER_READ` |
| `重复的权限码: ${code}` | 权限码唯一性 | YAML 中同一权限码出现多次 | 在 `contracts/permissions.yaml` 中删除或合并重复项 | 不要在 YAML 中写重复条目 |

#### C3. Contract Drift（`contracts-gen.js --check`）

触发命令：`node scripts/contracts-gen.js --check`

| 错误信息片段（原文） | 触发的检查项 | 根因分类 | 修复命令 | 预防建议 |
|---|---|---|---|---|
| `❌ Contract drift detected. Run \`pnpm run contracts:gen\` and commit the result.` | 生成代码与 YAML 不一致 | YAML 改了但未重新生成 Rust 代码 | `pnpm run contracts:gen && git add -A && git commit -m "fix: regenerate contracts after YAML change"` | 每次改 `contracts/error-codes.yaml` 或 `permissions.yaml` 后立即跑 `contracts-gen.js` |
| `Failed to generate Rust error codes` | 契约生成脚手架失败 | `contracts/codegen/gen-rust.js` 执行报错 | 检查 `contracts/error-codes.yaml` YAML 语法是否正确 | YAML 格式错误会导致 codegen 脚本 panic |
| `Failed to generate Rust permission codes` | 权限码生成失败 | `contracts/permissions.yaml` 格式错误 | 检查 YAML 语法和格式是否符合规范 | 同上 |

### D. 数据库命名类（`check-db-naming.js`）

触发命令：`node scripts/check-db-naming.js`

| 错误信息片段（原文） | 触发的检查项 | 根因分类 | 修复命令 | 预防建议 |
|---|---|---|---|---|
| `FAIL: ${errors.length} bare table name(s) found:` | 表名无服务前缀 | SQL 中用了裸表名 | 确认表名以服务前缀开头（`iam_`/`audit_`/`token_`/`ntf_`/`cfg_`/`file_`/`prue_`），或在 `contracts/db-conventions.md` 中申请新前缀 | 所有表必须带服务前缀；跨服务表走 RabbitMQ 事件，不要直连 |
| `${e.file}:${e.line} — table "${e.table}" has no prefix` | 具体裸表名定位 | 同上 | 找到对应文件行，将 `"${e.table}"` 改为 `"<prefix>${e.table}"` | 同上 |

## 高频失败 Top 15

按频率排序，最常见的放最前：

| # | 场景 | 触发脚本 | 修复命令 |
|---|---|---|---|
| 1 | 改了 `#[utoipa::path]` 但忘记重新生成 openapi.yaml | `git diff` (spec_generate) | `node scripts/dump-openapi-specs.js` |
| 2 | 在 Rust 代码加了新错误码但没同步到 `contracts/error-codes.yaml` | `check-spec.js` 或 `contracts-gen.js --check` | `pnpm run contracts:gen` |
| 3 | SQL 中用了裸表名（无 `iam_` 等前缀） | `check-db-naming.js` | 改 SQL 将 `"table_name"` 改为 `"<prefix>_table_name"` |
| 4 | 新建服务缺 Dockerfile | `check-service-standard.js` | 参照 `services/<svc>/Dockerfile` 补全 |
| 5 | 新建服务缺 tests/ 目录 | `check-service-standard.js` | 在 `services/<svc>/tests/` 下加 `*_test.rs` |
| 6 | 路由缺少 operationId | `check-spec.js` | 在 `#[utoipa::path]` 中加 `operation_id = "..."` |
| 7 | 改了 `contracts/permissions.yaml` 但忘记 `contracts-gen.js` | `contracts-gen.js --check` | `pnpm run contracts:gen` |
| 8 | 新建服务缺 AGENTS.md | `check-service-standard.js` | 参照已有服务 AGENTS.md 格式补全 |
| 9 | 新建服务缺 migrations/ 目录 | `check-service-standard.js` | 创建目录并补充迁移文件（用 `coder-migrate-db` skill） |
| 10 | 新建服务缺 README.md | `check-service-standard.js` | 参照 `services/token-service/README.md` 格式补全 |
| 11 | 错误码格式不符 `SERVICE_CATEGORY_NUMBER` | `check-spec.js` | 改 `contracts/error-codes.yaml` 中的码格式 |
| 12 | 权限码格式不符 `SERVICE_RESOURCE_ACTION` | `check-spec.js` | 改 `contracts/permissions.yaml` 中的码格式 |
| 13 | 错误码在 YAML 中重复 | `check-spec.js` | 删除或合并 `contracts/error-codes.yaml` 中的重复条目 |
| 14 | 权限码在 YAML 中重复 | `check-spec.js` | 删除或合并 `contracts/permissions.yaml` 中的重复条目 |
| 15 | 路由缺少 /health 或 /metrics 端点 | `check-service-standard.js` | 在路由或 main.rs 中注册对应端点 |

## 串行诊断决策树

当 CI 失败信息不明确时，按以下顺序排查：

```
1. 先问：CI 日志里哪一行是红色的（failed）？
   └─ 找第一个 exit 1 或非零退出的 job 名称

2. Job 名 → 对应脚本：

   service_standard_check 失败？
   └─ 跑: node scripts/check-service-standard.js
   └─ 看哪个 ⚠️ 缺失 项 → 补那个文件/端点/配置

   spec_check 失败？
   └─ 跑: node scripts/check-spec.js
   └─ 分类处理：
       - "缺少 operationId" → 补 operation_id
       - "错误码格式不正确" → 改 contracts/error-codes.yaml
       - "权限码格式不正确" → 改 contracts/permissions.yaml
       - "重复的错误码/权限码" → 去重

   contracts_check 失败？
   └─ 跑: node scripts/contracts-gen.js --check
   └─ 看 "Contract drift detected" → pnpm run contracts:gen

   spec_generate 失败？（git diff exit 非零）
   └─ 跑: node scripts/dump-openapi-specs.js
   └─ git diff 仍漂移 → node scripts/dump-openapi-specs.js && git add -A && git commit

   check-db-naming 失败？
   └─ 跑: node scripts/check-db-naming.js
   └─ 读输出中的 "table X has no prefix" → 改 SQL 表名

3. 多项同时失败时：
   优先级顺序：spec_generate（最快修复）→ contracts_check → spec_check → check-db-naming → service_standard_check
   原因：spec_generate drift 通常是最先触发的（改路由后忘记 dump）

4. 何时升用户：
   - 所有脚本都 ran 成功但 CI 仍然红 → 可能 GitLab runner 环境差异，请用户提供完整 CI 日志
   - Java SDK drift 失败（`check-java-sdk-drift.js`）且 `allow_failure: false` → 需要人工评估 SDK 侧 vs 服务侧修复方向
```

## 本地预检命令（一键复制）

push 前跑这 5 条，确认全部通过再推：

```bash
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
node scripts/dump-openapi-specs.js && git diff --exit-code -- 'services/*/openapi.yaml'
node scripts/check-db-naming.js
```

等价于项目根 `package.json` 中：

```bash
pnpm run check:service-standard && pnpm run check:spec && pnpm run contracts:check && pnpm run spec:generate && pnpm run check:db-naming
```

（`spec:generate` 后会自动 git diff drift，若 drift 有输出则退出码非零。）

## 验证

CI 失败修复后，在本地复验：

```bash
# 逐项验证
node scripts/check-service-standard.js        # 必须输出 "✅ 服务规范检查通过"
node scripts/check-spec.js                   # 必须输出 "✅ 所有契约规范检查通过"
node scripts/contracts-gen.js --check       # 必须输出 "✅ Contracts in sync"
node scripts/dump-openapi-specs.js
git diff --exit-code -- 'services/*/openapi.yaml'  # 退出码必须为 0
node scripts/check-db-naming.js             # 必须输出 "PASS: no bare table names found"
```

全部输出绿色（exit 0）后方可 push。
