# 09. AI 协作工作流

> AI Agent 在这个仓库怎么干活：Skills、L4 触发、Spec 纪律、Pre-commit。

---

## 9.1 Skills 速查

### 项目 Skill（`.agents/skills/`）

| Skill | 何时调用 |
|-------|---------|
| `coder-add-route` | 给现有服务加 endpoint |
| `coder-migrate-db` | 加数据库迁移（MySQL + SQLite） |
| `coder-new-service` | 从零搭一个新的 Rust 微服务 |

### 质量与诊断 Skill（`.agents/skills/`，按场景触发）

| Skill | 何时调用 |
|-------|---------|
| `coder-openapi-contracts-sync` | 改完路由后，跑 5 道契约门禁（dump spec → contracts 校对 → 跑 gates） |
| `coder-ci-diagnose` | CI 红灯时，按报错映射到根因 + 一键修复命令 |
| `coder-rust-unit-test` | 给 Rust service 加 `#[cfg(test)]` / testcontainers MySQL / wiremock / Principal 测试桩 |
| `coder-microservice-review-checklist` | PR 合规审查（X-User-* 禁用 / 表名前缀 / 错误码契约 / outbox 等 12 项 grep 必查） |
| `coder-perf-profile` | 慢/卡顿/内存涨时，做 CPU/内存火焰图、heaptrack、RSS baseline 对比 |
| `coder-contract-trace` | 跨服务事件追踪（RabbitMQ + transactional outbox 4 阶段定位「事件卡在哪」） |
| `coder-migration-rollback` | 线上迁移出错的应急回滚（forward-only 补偿迁移决策树） |

### 平台 Skill（`.trae/skills/`）

| Skill | 何时调用 |
|-------|---------|
| `coder-base-images-sync` | 同步基础 Docker 镜像 |
| `coder-dev-bootstrap` | 引导本地开发环境 |
| `coder-e2e-test` | 跑 E2E 测试 |
| `coder-lib-upgrade` | 升级共享 Rust 库 |
| `coder-service-audit` | 审计服务是否合规 |
| `plan-eng-review` | 工程经理模式审视计划 |

> 用 Skill 不是「可选」——它决定了 AI 走什么流程、产出什么格式。哪怕只 1% 相关性，也应该加载再看是否继续。

---

## 9.2 L4 触发器（自动加载）

AI Agent 会根据你触碰的路径自动加载 Skill：

| 路径 | 自动加载 |
|------|----------|
| `services/*/src/routes/*.rs` | `coder-add-route`（新增路由时），改完后跑 `coder-openapi-contracts-sync` |
| `services/*/migrations/**` | `coder-migrate-db`（新增迁移时） |
| `services/*/openapi.yaml` | **警告：禁止手改**，跑 `node scripts/dump-openapi-specs.js` |
| `contracts/**` | 跑 `node scripts/contracts-gen.js` 重新生成 |

完整触发映射见 [`.trae/rules/project_rules.md`](../../.trae/rules/project_rules.md)。

---

## 9.3 Spec 纪律（铁律）

**改代码前先有 spec**：

- 路径：`docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`
- 命名：日期 + 主题 + `-design.md` 后缀
- 阶段 commit：`Phase:<summary>` 或 `feat(scope): <summary>`

> 这是项目铁律，CI 不强制，但 reviewer 会强制。**没有 spec 的 PR 不进 merge**。

---

## 9.4 提交规范

- **任务分支优先**：新阶段功能 / 验证工作从 `main` 新建描述性分支（如 `validation/...`、`feat/...`）
- **一个任务一个 commit**
- **禁止 amend / force-push**
- Commit message：`<Phase>:<summary>` 或 `feat(scope): <summary>`
  - 例：`docs(plan): ...`、`fix: ...`、`feat(iam): add MFA login`

---

## 9.5 Pre-commit Hook

```bash
# 推荐：pre-commit 框架
pre-commit install

# 或者：零依赖兜底
cp scripts/git-hooks/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
```

Hook 自动跑：

- `service_standard_check`
- `spec_check`
- `contracts_check`
- `openapi_drift`

任何一项失败，commit 被拦截。

---

## 9.6 语言与风格

- **AI 思考 / 文档 / 对话回复**：中文
- **代码注释 / commit message / 技术术语**：英文
- 这条规则写入 `.trae/rules/project_rules.md §1.1`，全仓库（含 `.trae/rules/*`）统一执行

---

## 9.7 契约 / OpenAPI 自动生成

**改 YAML**（`contracts/error-codes.yaml`、`contracts/permissions.yaml`）：

```bash
node scripts/contracts-gen.js
# 生成 libs/digua_common-error/src/codes.rs
# 生成 libs/digua_authz/src/perms.rs
```

**改路由注解**（`#[utoipa::path]`）：

```bash
node scripts/dump-openapi-specs.js
# 生成 services/*/openapi.yaml
```

CI 用 `git diff --exit-code -- 'services/*/openapi.yaml'` 检测漂移。

> **绝对不要手改**这两个生成产物。

---

## 9.8 AI 速查卡片

| 我想 | 路径 |
|------|------|
| 走 AI 入口 | `docs-site/ai-context.md` |
| 跑服务标准审计 | `node scripts/check-service-standard.js` |
| 跑 spec 审计 | `node scripts/check-spec.js` |
| 跑契约审计 | `node scripts/contracts-gen.js --check` |
| 同步基础镜像 | `node scripts/base-images-sync.js` |
| 跑 E2E | `node scripts/run-e2e-local.js` |
| CI 失败诊断 | `.agents/skills/coder-ci-diagnose/SKILL.md` |
| PR 合规审查 | `.agents/skills/coder-microservice-review-checklist/SKILL.md` |
| 跨服务事件追踪 | `.agents/skills/coder-contract-trace/SKILL.md` |
| 性能/内存 profile | `.agents/skills/coder-perf-profile/SKILL.md` |
| 迁移应急回滚 | `.agents/skills/coder-migration-rollback/SKILL.md` |
