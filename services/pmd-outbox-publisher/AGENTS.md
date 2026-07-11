# pmd-outbox-publisher 服务规范

> Phase 9 PMD 服务专属 outbox publisher。**跨语言**：从 Java `project-service` 的 `OutboxWriter.enqueueWithLog(...)` 写出的 `pmd_outbox_events` 行里读，向 RabbitMQ `events` exchange（routing-key 前缀 `pmd.`）发布，最终被 `audit-service` 的 `audit.pmd-events` 队列消费。
>
> 仿 `services/iam-outbox-publisher/` 模板，与之独立部署（独立 Rust crate、同镜像不同 CMD 或独立 image）。

## 必需组件

- [ ] Dockerfile（豁免 — 容器镜像由 docker-compose 构建时复用 project-service 镜像，运行时切 CMD；或独立 image 见 Dockerfile.builder 模式）
- [x] AGENTS.md
- [ ] README.md（豁免 — 行为仅 ~185 行 main.rs，本文件即文档）
- [ ] tests/ 目录（豁免 — 单进程轮询，行为集成测试由下游 consumer + E2E 覆盖；新模块时按需补）
- [ ] /health 端点（豁免 — 无 HTTP；进程存活即健康，由 docker-compose `restart: unless-stopped` 守护）
- [ ] /metrics 端点（豁免 — 无 HTTP；Prometheus 通过 `process exporter` 抓容器内 PID 即可）
- [ ] migrations/ 目录（豁免 — 不持有 schema，全部表与 project-service / audit-service 共库 `superpowers`，由 project-service 迁移管理）
- [ ] Swagger 配置（豁免 — 无 HTTP）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `DATABASE_URL` | **必填** | MySQL 连接串。`project-service` 的 application.yml 用同一变量，docker-compose 注入相同值。**无 YAML fallback**——project-service 的 `config.yaml` 是 Spring Boot schema，不被 Rust serde_yaml 解析 |
| `EVENT_BUS_URL` | `amqp://guest:guest@localhost:5672/%2f` | RabbitMQ AMQP URI |
| `EVENT_BUS_EXCHANGE` | `events` | outbox publisher 投递目标 exchange（topic，由 `infra/config/rabbitmq/definitions.json` 声明） |
| `OUTBOX_POLL_INTERVAL_MS` | `200` | 空轮询 / 失败重试退避（毫秒，比 iam 更快） |
| `OUTBOX_BATCH_SIZE` | `100` | 单批 `claim_batch` 行数 |
| `RUST_LOG` | `info` | tracing 日志级别 |

## 二进制行为

| 阶段 | 说明 |
|------|------|
| 启动 | 初始化 tracing → 连 MySQL → fork SIGINT/SIGTERM handler |
| 主循环 | 串行：`MySqlOutboxRepo::claim_batch` → `RabbitMqPublisher::publish_raw` → `mark_published`。broker 失败时 `publisher = None` 触发下一轮重连 |
| 并发模型 | 多副本安全靠 `SELECT … FOR UPDATE SKIP LOCKED`（`MySqlOutboxRepo::claim_batch`） |
| 优雅退出 | SIGINT/SIGTERM → 设 `shutdown` 标志 → 当前批次 publish + mark 完成后退出 |
| 失败重试 | DB 错：日志 + 退避下一轮；broker 错：丢弃 `publisher`，下一轮重连；已 claim 但未 mark 的行下一轮会被重发（consumer 按 `event_id` 幂等） |

## 端点域

（无 HTTP — CLI 二进制）

## 表命名约定

仅读 `pmd_outbox_events` 表（envelope 列集由 Java 写入，与 `libs/common-event::outbox` `MySqlOutboxRepo::enqueue` 字段对齐）。所有 `pmd_*` 表由 `services/project-service/migrations/mysql/` 的 Flyway 迁移管理。CI 网关：`node scripts/check-db-naming.js`。

## 跨语言 schema 合约

写入端：`project-service` 的 `OutboxWriter.enqueueWithLog(...)`（同 `@Transactional` 内 JdbcTemplate 原生 SQL `INSERT INTO pmd_outbox_events`）。

读取端：本 binary 的 `MySqlOutboxRepo::claim_batch`（通过 `libs/common-event`）用 `SELECT id, event_id, routing_key, envelope_json WHERE published_at IS NULL` 拉未发布行。

任一端字段漂移会导致 0 claim 或 publish 后解析失败 → CI 端到端冒烟脚本会先暴露。

## 豁免

- Dockerfile
- README.md
- tests/ 目录
- /health 端点
- /metrics 端点
- migrations/ 目录
- Swagger 配置（SWAGGER_ENABLED 开关）

## AI 检查时机

修改 `services/pmd-outbox-publisher/` 下文件时自动检查
