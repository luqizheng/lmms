# ADR-007: 集中式日志方案选型

> 日期：2026-08-18
> 状态：**提议中**（草案）
> 关联：
> - [ai-context.md §4.2 Onboarding](../ai-context.md) — 待推进项的源头
> - [可观测性指南](../guides/observability.md)
> - [OTel 追踪指南](../guides/otel-tracing.md)
> - [Phase 6-A trace 链闭环 ADR 隐式约束](../ai-context.md) — OTel 是 trace 不收 logs

## 上下文

### 现状

平台 12 周路线图（Phase 1-10）+ Roadmap 2.0 已 [完成] 的可观测性栈**只覆盖指标 + 链路追踪，不覆盖日志**：

| 组件 | 收集什么 | 不收集什么 |
|---|---|---|
| Prometheus | 指标（counter/histogram/gauge） | 日志 |
| Grafana | dashboard 渲染 | 日志聚合搜索 |
| Alertmanager | 指标告警 | 日志告警 |
| Jaeger | 跨服务 trace UI | 日志 |
| OTel Collector | OTLP trace + metric（→ Jaeger / Prometheus） | 日志（未启用 OTLP logs） |

服务进程**全部输出 JSON 格式日志到 stdout**（`digua_common_telemetry::init_tracing()` 无条件 `.json()`，100% 服务统一调用），但**没有任何组件**在抓 stdout 入库。当前唯一聚合能力：

1. `docker logs <container>` 看单容器 stdout
2. 跨服务请求通过 `X-Trace-Id` response header 反查 Jaeger
3. 日志保留窗口 = 容器生命周期，重启即丢

### 痛点

`docs-site/ai-context.md §4.2 Onboarding` 已把"集中式日志收集方案（ELK / Loki / 阿里云 SLS）—— 待运维补充"列为 [待推进]。

具体场景下不可用：

- **用户报"我刚才那次登录报错"** → 只能让用户提供 trace_id（如果他们能从浏览器 devtools 看到 `X-Trace-Id`），无法按用户身份、租户、时间窗聚合
- **按租户排障** → 多租户（Phase 8 [完成]）已贯通，但日志无 `tenant_id` 标签字段，无法"看租户 X 过去 1 小时所有 4xx 响应"
- **审计追溯** → audit-service 持久化业务审计行（Phase 4 落地），但**应用层日志**无对应窗口，事件链断点
- **容量排障** → 想知道"过去 24h 哪个服务 ERROR 日志最多" → 现只能逐个 `docker logs | grep ERROR | wc -l`

### 为什么是现在

阻塞 1.0 后期真实用户接入（`ai-context §3` 路线图 R2.0 阻塞条件之一 = 至少 1 个外部租户实际接入）。租户接入后必然要求"我那笔操作日志给我看"——现在没有就要当场补。

### 已选型约束

- **运维预算低**：** 单 2.x 机器内存 ≤ 16GB，Loki 单体模式 RSS 100-300MB 可接受；ELK 单 ES 节点 ≥ 8GB 基本不够
- **K3s 化未上线**（Phase 7 [待推进]）：暂不上 Loki Operator / Helm chart，用 docker-compose overlay 即可
- **Grafana 已就位**：可零摩擦加 Loki datasource + derived fields 跳 Jaeger
- **JSON 已统一**：100% 服务输出 JSON stdout，**无需任何格式改造**

## 决议

### 总体方向

**Staging 先行 Loki + Promtail 单体模式；1.0 期间保持；2.0 阶段重新评估 ELK / 阿里云 SLS / 继续 Loki 三选一。**

分阶段：

| 阶段 | 时间 | 方案 | 触发切换条件 |
|---|---|---|---|
| **A. Staging 试点** | 2026-Q3 | Loki 3.3+ 单体 + Promtail 3.3+（docker_sd_configs） | 见 §实施路径 A1-A4 |
| **B. 1.0 期间** | 2026-Q4 | 沿用 A，叠加 MinIO 后端 + 30d 保留 | staging 稳定运行 ≥ 30d |
| **C. 2.0 重新评估** | 2027-H1 | ELK / 阿里云 SLS / 继续 Loki 三选一 | 满足 §2.0 决策点任一条件 |

### 为什么 Loki 而不是 ELK / SLS

| 维度 | Loki + Promtail | ELK（Elasticsearch + Logstash + Kibana） | 阿里云 SLS |
|---|---|---|---|
| 单体 RSS | **100-300MB** | 8GB+（ES 标配） | N/A（SaaS） |
| 磁盘开销 | 仅索引 label，正文 snappy 压缩（明文 1/3） | 全文索引（明文 3-5 倍） | 按量计费 |
| Grafana 集成 | 原生 datasource + derived fields | 需 Kibana 单独栈 | 需加 datasource |
| 运维复杂度 | 单体模式 1 容器 + 1 抓取器 | 至少 3 容器（ES/Logstash/Kibana），分片调优 | 零运维但供应商锁定 |
| 多租户隔离 | 原生 `X-Scope-OrgID` | ES ILM 手动实现 | RAM 授权 |
| 本地私有化 | 自托管 | 自托管 | 必须上云 |

**结论**：贴合本仓"低内存"主题 + Grafana 零摩擦 + 私有化友好 → Loki 是 staging 试点的最优解。

### 为什么 Promtail 而不是 Vector / Fluent Bit

| 维度 | Promtail | Vector | Fluent Bit |
|---|---|---|---|
| Loki 官方 | ✅ 一等公民 | 社区 | 社区 |
| docker_sd_configs | 原生 | 支持 | 支持 |
| pipeline_stages 表达力 | 强（cri + json + multiline） | 更强（VRL） | 中（Lua） |
| 资源占用 | 中 | 低（Rust 编译） | 低（C） |
| 与 Loki label schema 兼容 | ✅ 文档互引 | 需手写 transform | 需手写 filter |

**结论**：Loki 官方支持 + 文档完整 + 与 Loki label 语义最对齐 → Promtail。后续若需更高吞吐或 pipeline 复杂度可换 Vector（已在 Vector docs 收录 Loki sink）。

### 标签 Schema（核心承诺）

依据 Loki 官方原则 **"Labels are for routes, not for data"**：

| 标签 / 字段 | 类型 | Loki label？ | 来源 | 基数 |
|---|---|---|---|---|
| `service` | label | ✅ | 容器 label `service=<name>` 注入 | 低（11 种） |
| `level` | label | ✅ | JSON extract `level` 字段 | 低（4-5 种） |
| `env` | label | ✅ | 容器 label `env=staging` | 低（2-3 种） |
| `container` | label | ✅ | `__meta_docker_container_name` | 低 |
| `trace_id` | structured metadata | ❌ | JSON extract `fields.trace_id` | **高** |
| `tenant_id` | structured metadata | ❌ | JSON extract `fields.tenant_id` | **高** |
| `user_id` | structured metadata | ❌ | JSON extract `fields.user_id` | **高** |
| `request_id` | structured metadata | ❌ | JSON extract `fields.request_id` | **高** |
| `message` | full-text | ❌ | JSON 全文 | — |
| `timestamp` | ts | ❌ | RFC3339 | — |

**查询示例**：

```logql
# 按租户 + 错误级别 + 时间窗
{env="staging", service="iam-service", level="error"} 
  | json | tenant_id="tnt_abc123" 
  | line_format "{{.timestamp}} {{.message}}"

# 按 trace_id 关联跨服务日志
{env="staging"} 
  | json | trace_id="0af7651916cd43dd8448eb211c80319c"

# Prometheus alert on log rate（可选后续）
sum(rate({env="staging", level="error"}[5m])) by (service) > 1
```

### 保留窗口

| 环境 | 保留期 | 后端 | 配置 |
|---|---|---|---|
| Staging | **14d** | 本地 filesystem（`/loki` named volume） | `retention_period: 336h` |
| 1.0 生产 | **30d** | MinIO（S3 兼容） | `retention_period: 720h` + `delete_request_store: s3` |
| 2.0 | **90d** | 同上 + 冷归档 OSS | `retention_period: 2160h` |

**警告**：Compactor retention 和对象存储 lifecycle policy **不可同时只用其一**。lifecycle 过期时间须大于 `retention_period + delete_delay`，否则 Compactor 未清理完索引就触发删除导致查询失败。

### 2.0 决策点

满足以下任一条件时，**正式启动生产栈重选评估**：

1. Staging 跑 ≥ 30d，Loki 在 14d 保留 / 100MB/d 流量下出现查询延迟 > 5s 或丢日志
2. 合规要求 ≥ 90d 保留（Phase R2.0 阻塞条件之一"至少 1 个外部租户提出合规需求"）
3. Loki 单体模式 RSS > 800MB 或磁盘 > 50GB（容量告警）
4. 团队主动评估"换云厂商"（阿里云 SLS）或"换 ES"（需 Kibana 高级搜索能力）

届时按 ADR 标准流程做 ELK / 阿里云 SLS / 继续 Loki 三选一评估。

## 后果

### 好处

1. **可观测性闭环**：现有"指标 + trace" + 新增"日志"三件套齐全
2. **跨服务 trace 关联**：从 Jaeger trace → 跳到 Loki 日志（derived fields），按 `trace_id` 聚合所有相关日志
3. **按租户/用户排障**：`tenant_id` / `user_id` 作 structured metadata，配合 LogQL `| json | tenant_id="..."` 过滤
4. **代码最小侵入**：100% 服务已输出 JSON，**仅需在 `digua_common_telemetry::init_tracing()` 中加 trace_id 注入** + iam-service Cargo.toml 补 `otlp` feature
5. **复用 Grafana**：零新建 dashboard 框架，直接加 Loki datasource + 日志面板
6. **保留窗口可控**：14d / 30d / 90d 阶梯，对应运维成本可预期
7. **贴低内存主题**：单体模式 RSS 100-300MB，远低于 ELK

### 代价 / 风险

1. **一次性代码改动**（低风险）：
   - `digua_common_telemetry::init_tracing()` 加 `service.name` / `trace_id` 字段注入 → 影响所有 8 个 Rust service + 2 个 outbox publisher
   - `iam-service/Cargo.toml` 补 `digua_common_telemetry` 的 `otlp` feature（ai-context §1.1 已知漂移）

2. **Promtail 需 docker socket**：staging OK（已有 `/var/run/docker.sock` 挂载）；2.0 K3s 化后需换 Loki Operator + Promtail DaemonSet

3. **单点风险**：单体模式非 HA，staging 接受；生产前必须 MinIO 后端 + 至少 2 副本

4. **典型坑**（librarian 调研已识别）：
   - 中文日期 locale（Rust 默认 RFC3339 OK，无需处理）
   - JSON 多行堆栈（panic stack）→ Promtail `pipeline_stages.multiline.firstline` 配置
   - Promtail positions 文件需持久卷 → docker-compose 挂 `promtail-positions` named volume
   - Promtail 内存控制 → `batchsize: 1048576`（1MB 上限）

5. **新 CI 门禁**（可选）：服务 Dockerfile / compose 不允许非 JSON 日志输出（防退化）—— `scripts/check-log-format.sh` 扫所有 `tracing_subscriber::fmt` 调用点是否启用 `.json()`

### 风险缓解

| 风险 | 缓解 |
|---|---|
| iam-service 无 otlp → trace 断裂 | 阶段 A0 前置修复 |
| trace_id 未落 JSON 字段 | 阶段 A1 改 `init_tracing()`，加 `current_trace_id()` 注入 |
| Promtail 内存泄漏 | `batchsize` 上限 + 监控 RSS 指标 + Grafana panel |
| 容器重启日志丢 | Promtail positions 持久卷（`promtail-positions` named volume） |
| 中文 locale 时区 | Rust `tracing_subscriber` json 默认 RFC3339，无此问题 |

## 实施路径

### 阶段 A0 — 前置修复（半天）

**目标**：消除跨服务 trace 断裂

| 任务 | 文件 | 改动 |
|---|---|---|
| iam-service 补 otlp feature | `services/iam-service/Cargo.toml` | `digua_common_telemetry = { ..., features = ["otlp"] }` |
| 验证 iam-service 实际导出 | 启动后查 Jaeger UI | 应能看到 `iam-service` service 名 |

**验收**：Jaeger UI 服务列表含 `iam-service`。

### 阶段 A1 — trace_id 落 JSON 日志字段（半天）

**目标**：trace_id 进入日志字段，Promtail 可提取

| 任务 | 文件 | 改动 |
|---|---|---|
| init_tracing 加 service.name + trace_id 注入 | `libs/common-telemetry/src/lib.rs` | `Resource::new([KeyValue::new("service.name", service_name)])` + `with_current_subscriber().with(...)` |
| trace_request_middleware 写 span field | `libs/common-telemetry-axum/src/lib.rs` | 进入 span 时 `tracing::Span::current().record("trace_id", &trace_id)` |
| iam-service 同样处理（OTel 未启用时 fallback） | `libs/common-telemetry-axum/src/lib.rs` | 已有 random hex fallback 路径，复用 |
| 单元测试 | `libs/common-telemetry/tests/` | 验证 JSON 输出含 `trace_id` |

**验收**：任一服务启动后 `docker logs` 输出 JSON 含 `"trace_id"` 字段。

### 阶段 A2 — Loki + Promtail compose overlay（1 天）

**目标**：在 staging 起 Loki 单体 + Promtail，能采集所有业务服务日志

| 任务 | 文件 | 改动 |
|---|---|---|
| 新增 staging overlay | `infrastructure/docker-compose-observability.staging.yml` | 包含 loki + promtail + grafana-loki-datasource 三个 service |
| Loki 配置 | `infrastructure/config/loki/loki-staging.yaml` | 单体模式 + 14d 本地存储 + 100MB/d ingestion |
| Promtail 配置 | `infrastructure/config/loki/promtail-staging.yaml` | docker_sd_configs + cri + json + labels pipeline |
| 业务服务加 label | `infrastructure/docker-compose-services.yml` | 每个 service 加 `logging=loki` + `service=<name>` + `env=staging` 三个 label |
| staging 入口扩展 | `infrastructure/docker-compose.yml` | 主入口 include staging overlay（条件加载：`COMPOSE_FILE` env） |

**验收**：

```bash
docker compose -f infrastructure/docker-compose.yml \
  -f infrastructure/docker-compose-observability.staging.yml up -d
docker exec promtail wget -qO- http://localhost:9080/ready
# → 返回 "ready"
```

Loki UI（http://<staging-host>:3100）能查询到所有服务的 JSON 日志。

### 阶段 A3 — Grafana 集成（半天）

**目标**：复用现有 Grafana 加 Loki datasource + 日志面板

| 任务 | 文件 | 改动 |
|---|---|---|
| Loki datasource provisioning | `infrastructure/config/grafana/provisioning/datasources/loki.yaml` | 新增 |
| Derived fields（trace_id 跳 Jaeger） | 同上 | `matcherRegex: '"trace_id":"(\w+)"'`，URL → Jaeger UI |
| 日志面板示例 | `infrastructure/config/grafana/dashboards/logs.json` | 新增：按 service 过滤 + level 过滤 + 时间窗 |
| staging Grafana provisioning 加载 | `infrastructure/docker-compose-observability.staging.yml` | env 注入 provisioning 路径 |

**验收**：Grafana → Explore → Loki 数据源，能查 `{service="iam-service"} | json | tenant_id="tnt_xxx"`；点击日志的 trace_id 跳到 Jaeger。

### 阶段 A4 — 端到端验证 + Runbook（1 天）

**目标**：跑通"用户报登录报错 → 排障"完整路径

| 任务 | 文件 | 改动 |
|---|---|---|
| E2E 排障演练脚本 | `tests/e2e/log-traceability.test.ts` | 模拟用户报"我刚才登录报错"→ 用 tenant_id + 时间窗 → 查到对应请求日志 → 跳 Jaeger trace |
| Runbook | `docs-site/runbooks/log-aggregation.md` | 排障操作 SOP（5 步：搜日志 → 定位服务 → 看 span → 找用户上下文 → 确认根因） |
| 监控 | `infrastructure/config/prometheus/alerts.yml` | 新增 `LokiIngestionErrors` warning（基于 `loki_request_duration_seconds_count`） |
| 文档同步 | `docs-site/ai-context.md` | §6 可观测性表加 Loki/Promtail 行 + §4.2 关闭此 Onboarding 项 |

**验收**：

1. E2E 测试通过（5 分钟内）
2. Runbook 5 步操作可执行
3. ai-context §4.2 此项标 [完成]

### 阶段 B — 1.0 期间加固（2026-Q4）

- Loki 后端切 MinIO（30d 保留）
- Compactor 独立容器（参照 grafana/loki production compose 拓扑）
- Loki 多租户模式评估（`X-Scope-OrgID`，对外租户隔离）
- 增加应用层日志采样（高 QPS 服务）

### 阶段 C — 2.0 重新评估（2027-H1）

满足 §2.0 决策点任一条件时启动。新 ADR（ADR-008）覆盖。

## 验收标准

**Staging 阶段交付定义**：

| 项 | 标准 |
|---|---|
| 跨服务 trace 关联 | Loki 日志含 `trace_id` 字段；Grafana 点击 trace_id 跳 Jaeger |
| 按租户搜索 | LogQL `{service="iam-service"} \| json \| tenant_id="tnt_xxx"` 返回该租户全部日志 |
| 按用户搜索 | LogQL `\| json \| user_id="usr_xxx"` 返回该用户全部日志 |
| JSON 格式统一 | `scripts/check-log-format.sh` 扫描全库，0 个非 JSON 输出 |
| 保留窗口 | 14d staging + compactor 验证 retention 触发清理 |
| 资源消耗 | Loki 单体 RSS ≤ 300MB；Promtail RSS ≤ 100MB；磁盘 ≤ 50GB（14d × 100MB/d） |
| E2E 演练 | 5 分钟内完成"用户报报错 → 定位根因 → 跳 Jaeger trace" |
| Runbook | 5 步 SOP 可独立执行 |
| ai-context 同步 | §4.2 此项标 [完成]，§6 可观测性表更新 |

## 开放问题

1. **生产最终栈选型**：何时决断？建议：阶段 A 跑通 + staging 稳定 30d 后重新评估
2. **多租户隔离**：`X-Scope-OrgID` 是否启用？默认 staging 单租户（`tenant_id` 作 metadata 不作 label 即可）；生产评估
3. **是否替换 Promtail 为 Vector**？当前不需；如出现 pipeline 复杂度升级或 Loki 吞吐瓶颈再评估
4. **是否启用 OTLP logs 替代 Promtail**？OTel Collector 已支持 ` receivers.otlp.protocols.http` 接 logs；但 Promtail 更轻且 docker_sd_configs 更原生。**默认走 Promtail**，OTLP logs 作为 fallback 方案保留
5. **应用层日志采样**：高 QPS 服务（如 api-gateway）是否需要采样？默认不采样（staging 流量小），生产评估
6. **K3s 化后迁移路径**：Loki Operator + Promtail DaemonSet 是 K8s 标准答案，2.0 阶段处理

## 参考链接

### Loki / Promtail 官方文档

| # | 来源 | URL | 用途 |
|---|---|---|---|
| 1 | grafana/loki 主仓 production compose | https://github.com/grafana/loki/blob/main/production/docker/docker-compose.yaml | 单体 + compactor HA 拓扑 |
| 2 | grafana/loki loki.yaml (MinIO/S3 production) | https://github.com/grafana/loki/blob/main/production/docker/config/loki.yaml | S3 + compactor 完整配置 |
| 3 | grafana/loki promtail.yaml | https://github.com/grafana/loki/blob/main/production/docker/config/promtail.yaml | 单体模式静态文件采集 |
| 4 | ruanbekker/docker-promtail-loki (99★) | https://github.com/ruanbekker/docker-promtail-loki | docker_sd_configs 完整示例 |
| 5 | ruanbekker promtail.yaml | https://github.com/ruanbekker/docker-promtail-loki/blob/main/config/promtail/promtail.yaml | docker_sd_configs + cri + multiline 实战配置 |
| 6 | grafana/loki - Label best practices | https://github.com/grafana/loki/blob/main/docs/sources/get-started/labels/bp-labels.md | label cardinality 原则 |
| 7 | grafana/loki - Retention | https://github.com/grafana/loki/blob/main/docs/sources/operations/storage/retention.md | Compactor vs lifecycle policy |
| 8 | Grafana Loki docs - Monolithic mode | https://github.com/grafana/loki/blob/main/docs/sources/setup/install/helm/concepts.md | 三种部署模式对比 |

### 本仓相关文档

| # | 来源 | 用途 |
|---|---|---|
| 9 | [docs-site/ai-context.md §4.2](../ai-context.md) | 待推进项的源头 |
| 10 | [docs-site/ai-context.md §6](../ai-context.md) | 现有可观测性栈 |
| 11 | [docs-site/guides/observability.md](../guides/observability.md) | 可观测性指南（待补 Loki 章节） |
| 12 | [docs-site/guides/otel-tracing.md](../guides/otel-tracing.md) | OTel 追踪指南（trace_id 来源） |
| 13 | [Phase 6-A trace 链闭环 ADR](../ai-context.md) | 隐式约束：OTel 是 trace 不收 logs |
| 14 | [DB 命名规范契约](../../contracts/db-conventions.md) | 多租户 tenant_id 列设计参考 |