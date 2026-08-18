# 日志聚合故障排查 SOP（Staging Loki + Promtail）

> **状态**：可执行（Phase 6-A T2 落地后）
> **适用范围**：staging 环境 Loki 单 binary + Promtail docker_sd 采集
> **前置条件**：`docker compose` 已启动，`pnpm check:env` 通过；Loki / Promtail 已起来并加入 observability overlay
> **词法约定**：以下 `<compose>` 表示 `docker compose -f infrastructure/docker-compose.yml`

---

## 0. 一句话

- 5 步走：从 Grafana 可达 → 按租户过滤 → 定位错误日志 → 跳 Jaeger span → 关联 audit-service 事件表
- **没有自动告警到 Slack**（Alertmanager 当前只覆盖 metrics；日志告警见 R2.0）
- 配套脚本：`scripts/e2e-log-traceability.js`（CI 跑通这条链路）

---

## 1. 架构速览

| 组件 | 容器 / 端口 | 数据流 | 落地证据 |
|------|------|------|------|
| **业务服务** | `iam-service` / `token-service` / ... / `audit-service` | T1 commit `f04fa8e`：access log + 错误日志通过 `tracing_subscriber::fmt().json()` 走 stdout（CRI JSON） | `services/*/src/main.rs` 中 `tracing` JSON 初始化 |
| **Docker daemon** | host `/var/run/docker.sock` | Promtail 5s 周期刷 docker_sd | — |
| **Promtail** | 容器 `promtail`（同 compose overlay） | `docker_sd_configs` 过滤 `logging=loki` label → `cri: {}` 解析 → `json:` 抽出 `trace_id/tenant_id/user_id` → push 到 Loki | `infrastructure/config/loki/promtail-staging.yaml` |
| **Loki** | 容器 `loki` / 端口 `3100` | 单 binary + 本地 filesystem store（`/loki/chunks`），**14 天 retention**（无 S3 / 无 remote） | `infrastructure/config/loki/loki-staging.yaml` |
| **Grafana** | `localhost:3001` / datasource `Loki` | UI 查询入口；admin / admin | `infrastructure/config/grafana/datasources/` |

**关键约束**（来自 ADR-007）：
- **Loki labels 仅 `level / service / env`**（低基数；D3）。`trace_id / tenant_id / user_id` 全部走 JSON metadata，禁止升级为 label（会爆 stream cardinality）。
- Promtail 只采集带 `logging=loki` container label 的容器（opt-in；D2）。业务服务 compose 全部已带该 label。
- `loki-staging.yaml` 单 binary mode 不依赖 S3 / GCS——保证 overlay 轻量；**生产环境需要换 S3 backend**，见 R2.0 §1.2。

---

## 2. 前置条件

```bash
# 1. 确认 Loki / Promtail 容器运行
<compose> ps | grep -E 'loki|promtail'

# 期望输出：loki + promtail 都是 healthy / running 状态
# loki               ...   Up X minutes (healthy)
# promtail           ...   Up X minutes

# 2. 确认 Loki API 可达（默认端口 3100）
curl -sS http://localhost:3100/ready
# 期望：ready

# 3. 确认 Grafana datasource 已加载 Loki
# 浏览器：http://localhost:3001 → Connections → Data sources → Loki
# 期望：URL = http://loki:3100，Status = "default" / 绿色

# 4. 业务服务 stdout 是否走 JSON
# 任何服务的 main.rs 应当包含：
#   tracing_subscriber::fmt().json().with_env_filter(...).init();
# 见 scripts/check-log-format.sh（CI gate）
```

---

## 3. 5 步排查流程

### Step 1：确认 Grafana 可达 + datasource 加载

**目标**：排除 Grafana / Loki 自身可用性问题。

```bash
# Grafana 入口
open http://localhost:3001   # admin / admin（首次登录要求改密）
# 左侧菜单：Explore → datasource 下拉框选 "Loki"
```

**预期**：
- datasource 下拉框有 `Loki` 选项
- 输入 `{service="iam-service"}` 立刻出结果（实时查最近 5 分钟）

**失败定位**：
- datasource 缺失 → Grafana provisioning 没加载 `datasources/prometheus.yml`；重启 grafana 容器
- Loki 报 connection refused → 检查 `loki` 容器 network；compose 内默认网络 `coder-micro-services_default`

### Step 2：按租户过滤日志（多租户可追溯）

**目标**：定位"哪个 tenant 出问题"——这是 §8 关键约定第 14 条（多租户隔离）的可观测性兜底。

```logql
# 单租户 + 单服务（最常用）
{service="iam-service"} | json | tenant_id="tnt_xxx"

# 多服务
{service=~"iam-service|token-service|audit-service"} | json | tenant_id="tnt_xxx"

# 时间窗（默认查最近 5 分钟，加 since 自定义）
{service="iam-service"} | json | tenant_id="tnt_xxx" | since=1h
```

**预期**：
- 至少 1 条匹配行；每行 JSON 含 `tenant_id / trace_id / user_id / level / timestamp / message`
- tenant_id 为空 → 多租户校验失效（§8 关键约定违规），立即回 §4 关键约定第 14 条

**字段含义**：
| 字段 | 含义 | 来源 |
|------|------|------|
| `service` | service label（如 `iam-service`） | compose service name |
| `level` | `info` / `warn` / `error` / `debug` | `tracing::Level` |
| `env` | `dev` / `staging` / `prod` | compose env |
| `tenant_id` | `tnt_xxx` | JWT `tid` claim（T1 commit `f04fa8e` 后透传） |
| `trace_id` | W3C trace-id（32 hex） | access log 中间件注入 |
| `user_id` | `usr_xxx` | JWT `sub` |
| `timestamp` | RFC3339 | tracing_subscriber |

### Step 3：定位错误日志行（缩小范围）

```logql
# 全部 level=error
{service="iam-service", env="staging"} | json | level="error"

# 5xx 业务错误
{service="iam-service"} | json | status_code=~"5.."

# 含特定 message 子串（如 SQL 错误）
{service="iam-service"} | json | level="error" | message=~"(?i).*connection.*timeout.*"
```

**预期**：
- 错误行 `level="error"`，`status_code` 字段在 access log 存在（4xx / 5xx 自动标）
- 如果一行同时有 `status_code=500` 和 `trace_id`，是 gold 起点——下一步直接跳 Jaeger

**快速过滤技巧**：
- Grafana Explore 输入框支持管道 `| json | ...`，每加一段 stage 结果实时刷新
- 时间范围右上角选 `Last 5 minutes` / `Last 1 hour` / 自定义（自定义支持 ISO8601）
- 用 `|= "literal"` 做 substring 匹配（无需 `| json`）；`!~ "pattern"` 反向匹配

### Step 4：点击 trace_id 跳 Jaeger（cross-service trace 链接）

**目标**：从单条日志跳到完整跨服务 trace span 树。

```logql
# 先在 Loki 拿到目标 trace_id
{service="iam-service"} | json | tenant_id="tnt_xxx" | level="error"
```

- 复制日志行里的 `trace_id` 字段值（32 字符 hex）
- 浏览器新开 tab：`http://localhost:16686/search?traceId=<trace_id>`
- Jaeger 会渲染完整 span 树：api-gateway → iam-service → notification-service → outbox-publisher → audit-service

**预期**：
- Jaeger 搜到 ≥ 1 个 trace，且 span 数 ≥ 1
- 每个 span 上能看到 `service.name` / `http.method` / `http.status_code` 等属性

**失败定位**：
- Jaeger 搜不到 → 某环节 OTel 未启用（详见 ai-context §8 第 5 条）；compose 中 `OTEL_EXPORTER_OTLP_ENDPOINT` 没设 → 检查该服务的 compose env 块
- 搜到但只有 1 个 span → 跨服务 trace 链断在中间服务；查对应服务的 `tracing-opentelemetry` 注册代码

### Step 5：结合 audit-service 事件表定位根因（业务因果链）

**目标**：跨服务事件链路（Phase 4）涉及异步事件，Jaeger 只能展示 HTTP 同步调用，**业务因果靠 audit-service 事件表**。

```bash
# 直接进 mysql 容器查 audit_events 表
<compose> exec mysql mysql -uroot -p"${MYSQL_ROOT_PASSWORD}" \
  -e "SELECT id, event_type, trace_id, payload_json, created_at \
      FROM superpowers.audit_events \
      WHERE trace_id='<step4 拿到的 trace_id>' \
      ORDER BY id;"
```

**预期**：
- 至少 1 行 `audit_events` 命中该 trace_id
- 同一 trace_id 可能有多个 `event_type`（如 `iam.user.created` → `notification.announcement.published` → `audit.log.written`），按 `created_at` 升序拼出因果链

**关联技巧**：
- 找不到 → 该事件**未发**或 consumer 没 ack；查 `iam_outbox_events` / `ntf_outbox_events` 表的 `status` 列
- status=`pending` 长时间不更新 → outbox publisher 挂了；`pnpm run dev:logs iam-outbox-publisher`
- status=`published` 但 audit_events 没记录 → audit-service consumer 异常；查 RabbitMQ dead letter queue `events.dlx`

---

## 4. 常用 LogQL 速查

### 按 trace_id 精确反查
```logql
{service="iam-service"} | json | trace_id="aabbccddeeff00112233445566778899"
```

### 按 user 查全部活动
```logql
{service=~".+"} | json | user_id="usr_xxx" | since=24h
```

### 按错误级别聚合
```logql
sum by (service) (
  count_over_time({service=~".+", env="staging"} | json | level="error" [1h])
)
```

### 按 status_code 5xx 聚合
```logql
sum by (service) (
  count_over_time({service=~".+"} | json | status_code=~"5.." [5m])
)
```

### 按租户 + 时间窗（合规审计场景）
```logql
{service="iam-service"} | json | tenant_id="tnt_xxx" | since=7d | level=~"warn|error"
```

---

## 5. 故障自检清单

| 现象 | 根因定位 | 命令 |
|------|---------|------|
| Grafana 无 datasource | provisioning 没加载 | `docker compose restart grafana` |
| Loki `query_range` 报 404 | Loki 没起 / 端口错 | `curl http://localhost:3100/ready` |
| Loki 出结果但缺 `tenant_id` | 业务服务 tracing JSON 没注入 tenant_id | 看 main.rs；跑 `scripts/check-log-format.sh` |
| Promtail 没采集业务服务 | compose 缺 `logging=loki` label | `docker inspect <container> \| grep logging` |
| Jaeger 搜不到 trace_id | OTel 未启用或 compose env 没设 endpoint | ai-context §8 第 5 条 |
| Loki 卡死 / OOM | 单 binary 模式无对象存储；磁盘满 | `df -h /var/lib/docker/volumes/coder-micro-services_loki_data` |
| trace_id 在 Loki 有但 audit_events 没 | outbox publisher 挂 / consumer 没 ack | `pnpm run dev:logs iam-outbox-publisher` |

---

## 6. CI 演练命令

```bash
# 启动 Loki + Promtail 后跑完整 drill
<compose> up -d loki promtail
node scripts/e2e-log-traceability.js
# 退出码 0 = 链路 OK；非 0 = 失败（看上面「故障自检清单」）

# 仅守"日志必须 JSON"门禁（不需要 Loki 跑起来）
bash scripts/check-log-format.sh
```

---

## 7. 相关文档

| 场景 | 文档 |
|------|------|
| AI 协作入口 | [`docs-site/ai-context.md`](../ai-context.md) §6 可观测性 + §8 第 5 条 |
| ADR 集中式日志决策 | [`docs-site/decisions/ADR-007-centralized-logging.md`](../decisions/ADR-007-centralized-logging.md) |
| 跨服务事件追踪 | [`docs-site/runbooks/logging.md`](./logging.md)（现有 / 旧版） |
| OTel trace 接入 | [`docs-site/guides/otel-tracing.md`](../guides/otel-tracing.md) |
| 可观测性 stack 速查 | [`docs-site/guides/observability.md`](../guides/observability.md) |
| 备份与恢复 | [`docs-site/runbooks/backup.md`](./backup.md) |

> 本 SOP 与 `logging.md` 并存：旧版覆盖 logging 库选择；本 runbook 聚焦「**集中式日志聚合链路故障排查**」+ Loki 5 步走。新故障优先看本文件。