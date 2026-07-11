# Phase 9 — 软件项目管理系统（PMD）Java MVP 设计

> Status: Draft → Plan Approved Pending Worker
> Date: 2026-07-11
> Branch: `validation/software-project-management`
> Scope: `services/project-service/`（Spring Boot 3 + Java 25 + JPA）+ `services/pmd-outbox-publisher/`（Rust publisher 复用）+ contracts / gateway / audit / e2e

---

## 1. 背景

本阶段将软件项目管理系统作为新阶段验证项目，但业务服务本体改为 **Java**：`project-service` 使用 Spring Boot 3 + Java 25 + JPA 实现；事件 outbox publisher 继续复用 Rust 模板（`iam-outbox-publisher`），跨语言读取 Java 写入的 `pmd_outbox_events` 表。

这保留了现有微服务框架的核心能力验证：网关路由、contracts 生成、RabbitMQ topic exchange、audit-service 事件落库、MySQL 共享库表名前缀、CI gates；同时验证一个非 Rust 业务服务如何接入平台。

---

## 2. 已确认决策

| 项 | 决策 |
|---|---|
| 业务服务语言 | Java |
| Java 栈 | Spring Boot 3.3.x + Maven |
| JVM | Java 25 |
| DB 层 | JPA + 必要 native SQL/JdbcTemplate |
| Outbox publisher | 保留 Rust `pmd-outbox-publisher`，读 `pmd_outbox_events` |
| 表名前缀 | `pmd_` |
| LLM | `PlaceholderLlmProvider`，不接真实模型 |
| 前端 | MVP 不加 Vue 页面 |
| Gateway path | `/v1/project/*` |

---

## 3. 范围

### In scope

- `services/project-service/`：Spring Boot 3 Java 服务，容器内 8080、宿主 6008。
- `services/project-service/src/main/resources/db/migration/V001__init.sql` 与 `services/project-service/migrations/mysql/V001__init.sql` 同源，含：
  - `pmd_projects`
  - `pmd_milestones`
  - `pmd_tasks`
  - `pmd_sprint_mappings`
  - `pmd_outbox_events`
  - `pmd_llm_refinement_log`
- REST API：projects / milestones / tasks / sprints / llm refine / health / metrics / springdoc openapi。
- JPA entity/repository/service/controller 分层；所有查询显式带 `tenant_id`。
- `OutboxWriter` 用 JdbcTemplate native SQL 写 `pmd_outbox_events`，与业务写入同 `@Transactional`。
- `services/pmd-outbox-publisher/`：Rust binary，仿 `services/iam-outbox-publisher/`，轮询 `pmd_outbox_events` 发布到 RabbitMQ `events` exchange。
- `audit-service` additive 修改：新增 `audit.pmd-events` queue + `pmd.#` subscribe，复用 `persist()` 落 `audit_events`。
- contracts：db-conventions / permissions / error-codes / events 增加 PMD 条目。
- e2e：`tests/e2e/api/cases/project.yaml`，其中 1-2 case 标 `smoke: true`。

### Out of scope

- Vue frontend 页面。
- 真 LLM provider / Ark / Ollama / HTTP client。
- markdown 文档管理、CR、资源工时、甘特图、RAG、知识库。
- 修改既有业务服务迁移与核心业务路由；audit-service 仅允许 additive consumer subscribe。
- 用 Hibernate 自动建表（`ddl-auto` 必须 `validate`）。
- 用 JPA 写 outbox 行（必须 native SQL/JdbcTemplate）。

---

## 4. Java 服务结构

```text
services/project-service/
├── pom.xml
├── Dockerfile
├── AGENTS.md
├── README.md
├── migrations/mysql/V001__init.sql
├── src/main/java/com/kudi/coder/pmd/
│   ├── Application.java
│   ├── config/{OpenApiConfig,TenantContextFilter,MetricsConfig}.java
│   ├── domain/{Project,Milestone,Task,SprintMapping,TaskStatus}.java
│   ├── repo/{ProjectRepository,MilestoneRepository,TaskRepository,SprintRepository}.java
│   ├── service/{ProjectService,MilestoneService,TaskService,SprintService}.java
│   ├── outbox/OutboxWriter.java
│   ├── llm/{LlmProvider,PlaceholderLlmProvider,RefineCtx,RefinedRequirements,RefinedItem,LlmRefineController}.java
│   └── web/{ProjectController,MilestoneController,TaskController,SprintController,GlobalExceptionHandler}.java
└── src/main/resources/{application.yml,db/migration/V001__init.sql}
```

---

## 5. 事件与 outbox 合约

Java 业务服务写 outbox 行，Rust publisher 读同一张表。`pmd_outbox_events` 必须包含 publisher 需要的字段：

- `id`
- `event_id`
- `routing_key`
- `payload_json` / `payload`
- `trace_id`
- `tenant_id`
- `producer`
- `published_at`
- `attempts`
- `last_error`

Worker 必须以 `services/iam-outbox-publisher/src/main.rs` 与 `libs/common-event/src/outbox.rs` 为准，确保字段名精确匹配。若 Rust publisher 当前期望 `payload` 而 Java migration 写 `payload_json`，以 Rust publisher 实际 SQL 为准改 DDL 与 Java INSERT。

首批 routing keys：

- `pmd.project.created`
- `pmd.project.archived`
- `pmd.task.status_changed`
- `pmd.llm.refinement.requested`
- `pmd.llm.refinement.completed`

---

## 6. 验证标准

- `cd services/project-service && ./mvnw -DskipTests package` exit 0。
- `cd services/project-service && ./mvnw test` exit 0。
- `cargo check -p pmd-outbox-publisher` exit 0。
- `node scripts/check-db-naming.js` exit 0。
- `node scripts/contracts-gen.js --check` exit 0。
- `scripts/dump-openapi-specs.sh && git diff --exit-code -- 'services/*/openapi.yaml'` exit 0（Java service 通过 springdoc 抓取）。
- `pnpm -C tests/e2e test:smoke --grep project` exit 0。

---

## 7. 风险

| 风险 | 缓解 |
|---|---|
| Java 25 非 LTS | 本阶段为验证项目；若 CI 镜像缺 Java 25，worker 可切 Temurin 25 镜像并在 spec 里记录 |
| JPA 隐藏 SQL | 所有 tenant 查询必须显式 `findBy...AndTenantId`；`ddl-auto=validate`；outbox 用 JdbcTemplate |
| Rust publisher 读 Java 表 | DDL 与 Rust SQL 以 `iam-outbox-publisher` 实际字段为准；集成测试覆盖 |
| springdoc 与现有 dump-openapi 脚本不兼容 | 扩展 `scripts/dump-openapi-specs.sh` 对 Java service 走 curl 抓取 |

---

## 8. Worker 执行入口

执行计划见 `.omo/plans/project-mvp.md`。本 spec 取代早期 Rust 版 PMD MVP spec。若同时存在 `2026-07-11-pm-platform-design.md`，该文件视为更大范围平台草案，不作为本 MVP worker 的 source of truth。
