# project-service 服务规范

> Phase 9 验证 MVP 业务服务（Java / Spring Boot 3 / Java 25 / JPA / Flyway）。
> 与既有 7 个 Rust 服务并存于同一 MySQL `superpowers` 库（不写现有表，仅 own `pmd_*` 前缀）。
> Outbox publisher 仍由 Rust `services/pmd-outbox-publisher/` 提供（跨语言读 `pmd_outbox_events`）。

## 必需组件（8-item 平台标准）

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点（Spring Actuator，`management.endpoints.web.exposure.include: health,metrics,prometheus`）
- [x] /metrics 端点（Spring Actuator Prometheus，自定义 `service=project-service` tag）
- [x] migrations/ 目录（Flyway `classpath:db/migration` ↔ `services/project-service/migrations/mysql/` 同源）
- [x] Swagger 配置（springdoc-openapi，`springdoc.swagger-ui.path=/swagger-ui`、`springdoc.api-docs.path=/api-docs`）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `SERVER_PORT` | `8080` | Spring Boot 监听端口 |
| `DATABASE_URL` | `jdbc:mysql://localhost:3306/superpowers` | MySQL 连接（容器内通过 docker-compose 注入） |
| `DB_USER` / `DB_PASSWORD` | `root` / `rootpass` | dev 默认；生产从 secret 注入 |
| `SPRING_PROFILES_ACTIVE` | `dev` | Spring profile |
| `SWAGGER_ENABLED` | `true` | dev 默认；生产 `false`（springdoc 由 profile 控制） |
| `PMD_LLM_PROVIDER` | `placeholder` | `placeholder` / `http`（后续 phase 接入） |
| `EVENT_BUS_URL` | `amqp://guest:guest@localhost:5672/%2f` | RabbitMQ（outbox publisher 通过事件总线跨语言沟通） |
| `CONSUL_ADDR` | `http://localhost:8500` | Consul 服务发现 |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | 预留未启用 |

## 表与合约

| 项 | 说明 |
|---|---|
| 业务表前缀 | `pmd_`（见 `contracts/db-conventions.md`） |
| 共享表 | `pmd_outbox_events`（**写入** 由 Java `OutboxWriter.enqueueWithLog(...)` 同事务完成；**读取+发布** 由 Rust `pmd-outbox-publisher` `SELECT ... FOR UPDATE SKIP LOCKED` 完成） |
| 跨语言字段约束 | 列名 = Rust `MySqlOutboxRepo::enqueue` 入参；schema 漂移由 `scripts/check-db-naming.js` 拦截 + 测试容器覆盖 |
| 跨服务事件 topic | `events` exchange，routing-key 前缀 `pmd.`；`audit.pmd-events` 队列订阅 `pmd.#`，由 `audit-service` 落 `audit_events` |

## 端点域

| 端点 | 方法 | 说明 |
|---|---|---|
| `/project/health` | GET | liveness + readiness（Actuator） |
| `/project/metrics` | GET | Prometheus scrape（Actuator `/metrics` 由 gateway 命名空间去掉 `/v1/project`） |
| `/project/projects` | POST / GET | 项目创建 / 列表 |
| `/project/projects/{id}` | GET / PATCH / DELETE | 项目详情 / 编辑 / 归档（软删） |
| `/project/projects/{id}/milestones` | POST | 子资源 milestone 创建 |
| `/project/projects/{id}/tasks` | POST | 子资源 task 创建 |
| `/project/tasks/{id}` | GET / PATCH / DELETE | 任务详情 / **状态机转移** / 软删 |
| `/project/sprints` | POST / GET | sprint 列表 + 创建 |
| `/project/llm/refine` | POST | LLM 细化需求（占位 provider；同事务写 outbox + log） |
| `/swagger-ui` | GET | SpringDoc UI（`SWAGGER_ENABLED=true` 时） |
| `/api-docs` | GET | OpenAPI yaml/json |

## 内部端点 /pmd/internal/*

| 端点 | 方法 | 说明 |
|---|---|---|
| `/pmd/internal/migrations/state` | GET | Flyway schema 状态查询 |

## 多租户隔离

- HTTP handler 起手调用 `TenantContext.requireTenantId()`，缺失或与 DB 行 `tenant_id` 不一致 → 404；
- 所有 JPA repo 方法派生方法带 `tenantId` 形参（如 `findByIdAndTenantId`、`findAllByTenantId(pageable)`），不允许裸 `findAll`/`findById`；
- `pmd_outbox_events` 写入同事务携带 `tenant_id`（`OutboxWriter.enqueueWithLog`）。

## 豁免

- 暂无（与既有 8-item 全部对齐）。

## 已知边界

- JPA `ddl-auto: validate` 仅验证 schema 与 entity 匹配；DDL 来自 Flyway `V001__init.sql`，**禁止** `update` / `create`。
- outbox 行**必须**经 `@Transactional` + `JdbcTemplate` 原生 SQL 写入；不走 JPA `EntityManager.persist` 以保跨语言字段稳定性。
- LLM 占位实现仅返回固定 `RefinedRequirements`；HTTP provider 等后续 phase。
