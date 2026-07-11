# Phase 9 — 新阶段验证项目：软件项目管理系统（PMD）MVP 设计

> Status: Draft
> Date: 2026-07-11
> Branch: `validation/software-project-management`（从 `main` 拉出）
> Predecessor: 仓库刚完成 Phase 8（IAM 多租户 + 韧性三件套），本 spec 进入下一阶段验证。
> Companion spec: `2026-07-11-pmd-mvp-design.md`（本文件）+ 后续实施 plan `2026-07-11-pmd-mvp-plan.md`

---

## Context

仓库已经稳定服务 8 个业务后端服务（api-gateway + token + iam + notification + audit + config + file + prue_backend），并完成 Phase 4 事件总线闭环、Phase 8 多租户加固、Phase 5B-FU3 投权同步等。`docs/superpowers/specs/2026-06-25-roadmap-completion-summary.md` 标记 12 周路线图全部交付。

下一步需要重新启动新阶段：构建一个**软件项目管理系统**作为端到端验证项目，验证本框架对「领域 CRUD + 事件总线 + 多租户 + 审计 + LLM 接入」五件套的支持面。本 spec 描述 MVP 形态。

> **决策记录（ADR-003 候补）**：本阶段从「不进 feature 分支」切到「feature 分支优先」，并在 `.trae/rules/project_rules.md` §1.2、AGENTS.md 等 5 处 AI 入口文档同步落地（commit `b0c6b0a`，2026-07-11）。原因：MVP 会改变服务数 + 路由 + 编排，需要隔离分支便于回退。

---

## Goals

1. 在不破坏现有 8 个服务的前提下，新增一个 **MVP 级**业务服务，承担「项目 → 里程碑 → 任务」三层结构的核心 CRUD。
2. 验证事件总线**第二次**接入：project-service 生产 → audit-service 落库（`audit_events`）的链路贯通，可重放至另一 consumer 的形态。
3. 验证**多租户**隔离（`tenant_id` 注入、点查询 `WHERE tenant_id = ?` 全覆盖）与**审计**字段（actor / request_id / trace_id）的端到端。
4. 验证 **LLM 接入抽象**的可替换形态：MVP 不接真模型，只用 `PlaceholderLlmProvider` 占位 + 一个能验证事件 + 落库的最小实现。后端调真模型要等 MVP 落地后再开新 spec。
5. **CI gates 全过**：`node scripts/check-service-standard.js`、`scripts/check-spec.js`、`scripts/contracts-gen.js --check`、`scripts/dump-openapi-specs.sh && git diff --exit-code -- 'services/*/openapi.yaml'`、`node scripts/run-tests-docker.js project-service`、`pnpm -C tests/e2e test:smoke`（新增 project-service 的 smoke 用例）。
6. **不增加前端工作量**：MVP 不实现 Vue 页面，仅在 admin swagger 里可见 + e2e API 用例覆盖。

---

## Components

### 服务级（新增 1 个）

| 名称 | 形态 | 端口 | DB | 事件 | Outbox | 消费者 |
|---|---|---|---|---|---|---|
| `project-service` (`pmd`) | Rust + Axum，仿 notification-service | 8080 内 / 6008 外（docker-compose） | MySQL 共库 `superpowers`，前缀 `pmd_` | 生产 `pmd.*` | ✅ 同 iam-outbox-publisher 模式，独立 bin `pmd-outbox-publisher` | ❌（MVP 不消费事件） |

### 共享层 / 编辑层（修改既有）

| 文件 | 改动 |
|---|---|
| `Cargo.toml`（workspace root） | 加 `services/project-service` 与 `services/pmd-outbox-publisher` 为 members |
| `contracts/db-conventions.md` | 表名前缀映射表格加 `project-service` → `pmd_` |
| `contracts/permissions.yaml` | 加 `PMD_PROJECT_*`、`PMD_MILESTONE_*`、`PMD_TASK_*`、`PMD_LLM_REFINE` |
| `contracts/error-codes.yaml` | 加 `PMD_*` 系列（NotFound / Conflict / Forbidden） |
| `contracts/events.yaml` | 加首批 `pmd.*` 事件：`project.created`、`project.archived`、`task.status_changed`、`llm.refinement.requested`、`llm.refinement.completed` |
| `infrastructure/config/rabbitmq/definitions.json` | 新增 queue `audit.pmd-events`、binding 路由键 `pmd.#` |
| `infrastructure/config/api-gateway/routes/project.router.toml` | 网关代理 `/v1/project/*` 与 `/v1/project/health` |
| `infrastructure/docker-compose*.yml` | project-service + pmd-outbox-publisher 接入，端口映射 |
| `services/audit-service/src/consumer.rs` | 新增对 `audit.pmd-events` 的消费，handler 复用同 `audit_events` 落库（沿用 `persist`） |
| `services/audit-service/src/main.rs` | 多订阅：`try_subscribe` 现有 `iam.#` + 新增 `pmd.#` |
| `services/iam-outbox-publisher/` | （不改）作为模板参考 |
| `.agents/skills/coder-new-service` | （不改）作为脚手架参考 |

### 测试 / 验收

| 文件 | 形态 |
|---|---|
| `services/project-service/tests/` | Rust 单测 + testcontainers 集成测试 |
| `tests/e2e/api/cases/project.yaml` | Vitest + supertest：列出 8 个 happy-path 用例（创建/查询/迭代/任务/状态/LLM 走 placeholder 验证入库 + outbox）|
| `tests/e2e/smoke/` | 标记 `smoke: true`，新增 1-2 个 project 用例 |
| `docs/guides/api-gateway-routing.md` | 同步 `project.router.toml` 示例 |

---

## Key Architectural Decisions

### D1：单服务脚手架，不拆多服务

用户已确认「单服务脚手架 (Recommended)」。理由：

- 验证需求是「五件套能用」，不是领域独立。每个真实业务限界上下文（项目 / 任务 / 里程碑）体量都太小，拆三个服务只是把 8 项服务标准 ×3。
- 单服务 + 多张表足够演示多租户、状态机、事件、审计。

### D2：表名前缀 `pmd_`（project management demo）

不沿用 `iam_/audit_/token_/ntf_/cfg_/file_/prue_` 中的任何一个，原因：

- 都不是项目域。
- `prj_` 已经接近 `prue_`，易混淆。
- 用户在「marker 阶段产物」语境下倾向保留「pmd_」（占位，待用户否决）。

> **争议点**：是否要让 `pmd_` 跟现有的短服务名 `ntf_/cfg_/file_/iam_` 对齐换成 `project_`？前者更短但和现有 3 字母不规范；后者更可读但 7 字母可能超 audit_events 表前缀字段长度。本 spec 默认 `pmd_`，用户可在审批时覆盖。

### D3：4 张表，不上 markdown / CR / 工作表

用户已确认「现在只上 project+milestone+task」。后续「里程碑 + 甘特」「CR」「资源 + 工时表」「markdown」「LLM」被砍到后续 phase，原因：

- 每加一类需要至少 1 张表 + 1 个 service 层级 + 1 套校验。MVP 只锁「四件套贯通的最小路径」。
- markdown / CR / 工时需要先起一份导引 spec；本 spec 把它们列入「下阶段」。

### D4：LLM 占位 + 可替换

`LlmProvider` trait + `PlaceholderLlmProvider` + 路由 `POST /v1/project/llm/refine`，handler：

- 接受原始需求文本（自由字符串，最大 16 KiB）。
- 在同一 sqlx transaction 里：
  - 写 `pmd_llm_refinement_log`（含 request / response / caller / project_id）；
  - 调用 `common_event::MySqlOutboxRepo::enqueue(&mut tx, &Envelope::new("project-service", payload))`。
- placeholder 返回固定 `RefinedRequirements { items: [...] }`，记日志。
- trait 后可换：HTTP-based 实现（火山 Ark / Ollama）只换 DI 实例，业务代码不动。

> **争议点**：是否要 MVP 中就预设 HTTP provider 并打 `LLM_HTTP_BASE_URL` env，等同于「留 stub 的 stub」？本 spec 选占位，理由：**MVP 不引入新外部依赖**（与 D1 同源）。

### D5：事件总线接 audit-service

延续 Phase 4.1 的 `audit.iam-events` 模式：新建 `audit.pmd-events` queue，binding `pmd.#`，audit-service 在 `try_subscribe` 中加一份订阅、复用同一个 `persist()` handler（落 `audit_events`）。

> 这么做不引入「路由键分发」复杂度：production 端用 `pmd.*`，consumption 端所有事件都走同一张表（`audit_events`）。

### D6：outbox publisher 独立 binary

参照 `services/iam-outbox-publisher/`，新增 `services/pmd-outbox-publisher/`，同镜像切 CMD。`Cargo.toml` workspace member +1；`docker-compose.yml` 多一个服务。

### D7：CI gates 在 PR review 阶段必跑

按 `.trae/rules/project_rules.md` §1.5 + 新分支规则（commit `b0c6b0a`）：

- push 前跑：`scripts/check-service-standard.js`、`scripts/check-spec.js`、`scripts/contracts-gen.js --check`、`scripts/dump-openapi-specs.sh && git diff --exit-code -- 'services/*/openapi.yaml'`、`node scripts/run-tests-docker.js project-service`。
- `pnpm -C tests/e2e test:smoke` 需标记 `smoke: true`，CI smoke 子集约 2 min。

### D8：状态机

`pmd_tasks.status` 用枚举：`backlog / todo / in_progress / blocked / review / done / cancelled`。变更只能从合法状态推进（如 `done` 不能直接回 `backlog`）。本阶段用 Rust enum + 强类型校验，不引 `statemachine` 库。

---

## Why Stage 9 而不是 Stage 0

新阶段验证的目的不是「交付一个能卖给客户的项目管理系统」（那是后续 product 阶段），而是「确认当前微服务框架 + CI gate + 事件总线 + 多租户 + 审计 + 日志 + 文档」的真实负载能力。本 MVP 故意保持 4 张表 + 占位 LLM，把「验证阈值」压在「一次性 spec + 一次性 plan + 一次性 scaffold + 一次性契约 gen + 一次性 gate run」走通的最小单元。

---

## Verification Strategy

按 ulw-plan `references/full-workflow.md` Phase 0/Phase 2/Phase 3 + `.trae/rules/project_rules.md` §1.5：

1. **Spec 写入**：先有本 spec（已落 `docs/superpowers/specs/2026-07-11-pmd-mvp-design.md`），后写代码。
2. **Plan 写入**：写 `docs/superpowers/plans/2026-07-11-pmd-mvp-plan.md`，7 个 todos（含 spec / scaffold / contract / migration / route + handler / outbox publisher + audit consumer / gates + smoke）。
3. **Execution（另一 worker session 跑）**：本 plan 由 worker 跑，每个 todo 一个 commit。
4. **验收（worker 内）**：每个 todo 跑 `node scripts/check-service-standard.js` + `node scripts/check-spec.js` + `node scripts/contracts-gen.js --check` + `scripts/dump-openapi-specs.sh && git diff --exit-code -- 'services/*/openapi.yaml'` + `node scripts/run-tests-docker.js project-service` + `pnpm -C tests/e2e test:smoke`。
5. **Final verification wave**：F1-F4 平行 pass。

---

## Risks & Mitigations

| 风险 | 缓解 |
|---|---|
| `pmd_` 不是现有短前缀之一，对前端、i18n 都很陌生 | spec 内 D2 已点名候选，需用户在审批点确认或驳回 |
| 新服务会冲击 docker-compose / 路由表 / events 拓扑 | 全部走模板 + idempotent；git diff 一次确认 |
| LLM 占位被误读为「能调 LLM」 | README 明确「MVP 仅占位」，env 命名 `LLM_PROVIDER=placeholder\|http` 留扩展位 |
| audit-service 复用 persist handler 出 bug | 单测覆盖「`pmd.user.created` 落库」与「二次重放幂等」（`ON DUPLICATE KEY UPDATE` 路径在 V001__init.sql 第 60-66 行）|
| 迁移顺序导致 CI 漂移 | forward-only；本 PR 不修改既有 V001-00N，只新增 `services/project-service/migrations/mysql/V001__init.sql` |
| 用户期望找 markdown / CR / 工时却没找到 | spec D3 已明确为「下阶段」，TL;DR 中「What it will NOT do」会显眼列出 |
| e2e smoke 用例挤爆子集 | 仅引入 1-2 个标 `smoke: true` 的用例；项目 service 完整用例退到 `test:api` |

---

## Out of Scope（明确不做）

- **Frontend 页面**：Vue 端不增加；Swagger UI + admin swagger 是唯一直观入口。
- **跨租户 super-admin UI / API**：仅扩展现有 iam CrossTenant 能力；MVP 不提供 operator API。
- **实时协作 / WebSocket**：MVP 不接 notification-service 的 WS。
- **统计 / 报表**：不做 dashboard 不做 worklog 报表。
- **集成 file-service 附件**：不上传附件；LPM 需求文本走 DB text 列。
- **真 LLM 接入**：占位 `PlaceholderLlmProvider` 已是 MVP 最大化；HTTP provider 要后续 spec。
- **markdown 文档管理**：纯 markdown 设计 → 下一阶段 spec。
- **变更评审（CR）**：下一阶段 spec。
- **资源 + 工时表**：下一阶段 spec。
- **里程碑甘特**：下一阶段 spec。
- **RAG / 知识库**：下一阶段 spec。
- **运维面板 / 业务监控面板**：复用现有 Prometheus + Grafana，不加新 dashboard。
- **CI/CD 灰度**：用 GitLab CI 标准流程，不人为切 canary（不在本 PR 范围）。

---

## Deliverables

仓库 PR 一个，commit 序列（worker 跑）：

1. `chore(repo): turn on task-branch-first rule`（已落，commit `b0c6b0a`）。
2. `docs(spec): add pmd mvp design for stage-9 validation`（本 spec）。
3. `feat(contracts): add pmd permission/error/event codes and table prefix`。
4. `feat(project-service): scaffold service skeleton + Docker + AGENTS + README + openapi bin`。
5. `feat(project-service): add pmd migrations (projects / milestones / tasks / sprint_mappings / outbox / llm log)`。
6. `feat(project-service): implement project/milestone/task routes + handler tests`。
7. `feat(project-service): add pmd-outbox-publisher bin + RabbitMQ topology wiring`。
8. `feat(audit): consume pmd.# events into audit_events`。
9. `feat(gateway): route /v1/project/* to project-service`。
10. `test(e2e): add project-service smoke + api cases`。
11. `chore(runtime): add project-service + pmd-outbox-publisher to docker-compose`。
12. `docs(guide): sync api-gateway-routing example with project.router.toml`。

> **不是所有 12 个 commit 都在 worker 单轮里一次性交付**；按 ulw-plan wave 拆分。worker 在收到明确 start 信号后开干。

---

## Acceptance Criteria（worker 完成验证）

```text
[ ] 工作分支 = validation/software-project-management
[ ] 服务脚本与文档均按本 spec 落地
[ ] Cargo.toml workspace members 含 services/project-service + services/pmd-outbox-publisher
[ ] services/project-service/Cargo.toml 含 dump-openapi bin
[ ] contracts/db-conventions.md 加 pmd_ 前缀
[ ] contracts/{permissions,error-codes,events}.yaml 加首批 pmd 条目
[ ] node scripts/contracts-gen.js 重新生成 libs/common-error/src/codes.rs 与 libs/authz/src/perms.rs
[ ] infrastructure/config/rabbitmq/definitions.json 加 audit.pmd-events queue 与 pmd.# binding
[ ] services/audit-service/src/consumer.rs 新订阅 pmd.# 与既有 iam.# 并存
[ ] services/project-service/migrations/mysql/V001__init.sql 含 4 张业务表 + pmd_outbox_events + pmd_llm_refinement_log + 索引
[ ] node scripts/check-service-standard.js  exit 0
[ ] node scripts/check-spec.js             exit 0
[ ] node scripts/contracts-gen.js --check  exit 0
[ ] scripts/dump-openapi-specs.sh          后 services/project-service/openapi.yaml 已生成且 git diff 干净
[ ] node scripts/run-tests-docker.js project-service 测试全过（含 LLM placeholder 单元测试）
[ ] docker compose -f infrastructure/docker-compose.yml up -d project-service + pmd-outbox-publisher 起得来
[ ] pnpm -C tests/e2e test:smoke 含新 1-2 用例且全过
```

---

## Open Questions / 需用户拍板

1. **服务表前缀**：默认 `pmd_`（占位）。是否要换成 `project_`（可读）或 `pjm_`（短）？
2. **MVP 是否包含最小前端**：当前 spec 默认「否」。是否要在 admin swagger 之外补一个「项目列表」极简页（仅 admin 用）？

> 以上 1-2 在审批 plan 时一并确认。

---

> 本 spec 是 Phase 9 验证项目的入口设计。下面由 `docs/superpowers/plans/2026-07-11-pmd-mvp-plan.md`（worker 写）落实为可执行 todos。
