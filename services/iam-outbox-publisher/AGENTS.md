# iam-outbox-publisher 服务规范

> 同 iam-service Docker 镜像、独立 CMD 的 CLI 二进制。轮询 `iam_outbox_events` 表，把未发布行投递到 RabbitMQ。

## 必需组件

- [ ] Dockerfile（豁免 — 复用 iam-service 镜像，docker-compose 中 `command: ["outbox-publisher"]`）
- [x] AGENTS.md
- [ ] README.md（豁免 — 行为仅 ~190 行 main.rs + outbox_archive 子模块，本文件即文档）
- [ ] tests/ 目录（豁免 — 单进程轮询，行为集成测试由下游 consumer + E2E 覆盖；新模块时按需补）
- [ ] /health 端点（豁免 — 无 HTTP；进程存活即健康，由 docker-compose `restart: unless-stopped` 守护）
- [ ] /metrics 端点（豁免 — 无 HTTP；Prometheus 通过 `process exporter` 抓容器内 `iam-outbox-publisher` PID 即可）
- [ ] migrations/ 目录（豁免 — 不持有 schema，全部表与 `iam_users` / `iam_outbox_events` 同库 `superpowers`，由 iam-service 迁移管理）
- [ ] Swagger 配置（豁免 — 无 HTTP）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | （必填或 CONFIG_PATH 兜底）| 数据库连接串（`mysql://` 或 `postgres://`，按 URL 前缀判方言）（iam-service 共享库 `superpowers`） |
| CONFIG_PATH | `config/config.yaml` | `DATABASE_URL` 未设时，从 iam-service config YAML 兜底读 `database.url` |
| EVENT_BUS_URL | `amqp://guest:guest@localhost:5672/%2f` | RabbitMQ AMQP URI |
| EVENT_BUS_EXCHANGE | `events` | outbox publisher 投递目标 exchange（topic） |
| OUTBOX_POLL_INTERVAL_MS | `1000` | 空轮询 / 失败重试退避（毫秒） |
| OUTBOX_BATCH_SIZE | `100` | 单批 `claim_batch` 行数 |
| RUST_LOG | `info` | tracing 日志级别 |

## 二进制行为（Phase 6-A T2-fu3：从 iam-service 拆出）

| 阶段 | 说明 |
|------|------|
| 启动 | 初始化 tracing → 连 MySQL → fork `outbox_archive` cron 任务 → fork SIGINT/SIGTERM handler |
| 主循环 | 串行：`claim_batch` → `RabbitMqPublisher::publish_raw` → `mark_published`。broker 失败时 `publisher = None` 触发下一轮重连 |
| 并发模型 | 多副本安全靠 `SELECT … FOR UPDATE SKIP LOCKED`（`MySqlOutboxRepo::claim_batch`）；archive cron 靠 MySQL `GET_LOCK` |
| 优雅退出 | SIGINT/SIGTERM → 设 `shutdown` 标志 → 当前批次 publish + mark 完成后退出；不会留下"已发但未 mark"的脏行 |
| 失败重试 | DB 错：日志 + 退避下一轮。broker 错：丢弃 `publisher`，下一轮重连。已 claim 但未 mark 的行下一轮会被重发（consumer 按 `event_id` 幂等去重） |

## 端点域

（无 HTTP — CLI 二进制）

## 表命名约定

复用 iam-service 的表：`iam_outbox_events`、`iam_users`（archive cron 仅读）。所有 `iam_*` 表由 iam-service 的 `migrations/` 维护。CI 网关：`node scripts/check-db-naming.js`。

## 豁免

- Dockerfile
- README.md
- tests/ 目录
- /health 端点
- /metrics 端点
- migrations/ 目录
- Swagger 配置（SWAGGER_ENABLED 开关）

## AI 检查时机

修改 services/iam-outbox-publisher/ 下文件时自动检查