# SLO / SLI 定义

> 更新时间：2026-07-29

本文档定义了 Coder Micro Services 平台所有服务的 Service Level Objectives (SLO) 与 Service Level Indicators (SLI)。所有告警规则、错误预算计算和 on-call 响应策略均以此文档为权威依据。

---

## 1. 总则

### 1.1 滚动窗口

所有 SLO 基于 **28 天滚动窗口 (rolling window)** 计算。不采用日历月 (calendar month)，原因如下：

- 滚动窗口在任意时刻都覆盖最近 28 天的数据，不受月初/月末边界效应干扰
- 与 Prometheus `[28d]` range vector 语义直接对齐，查询无需额外逻辑

### 1.2 错误预算 (Error Budget)

错误预算定义为窗口内允许的最大不可用时间：

```
Error Budget = (1 - SLO) * 窗口总时长
```

28 天窗口 = 28 * 24 * 3600 = 2,419,200 秒。

### 1.3 烧钱速率 (Burn Rate) 告警阈值

Burn rate 以**每月预算消耗比例**衡量。告警分两级：

| 阈值 | 含义 | 响应 |
|------|------|------|
| Burn Rate 超过每月预算的 **50%** | Warning — 需关注并评估趋势 | 值班工程师评估、记录 incident |
| Burn Rate 超过每月预算的 **80%** | Critical — 即将耗尽，需立即响应 | 升级到 on-call、启动 incident response |

> 烧钱速率由 Prometheus recording rule 自动计算，表达式见 [6. 数据来源与查询](#6-数据来源与查询)。

### 1.4 服务分级

整个平台服务按业务关键性分为两个 tier：

| Tier | 可用性 SLO | 适用服务 |
|------|-----------|----------|
| **Core** | 99.9% (三个九) | 核心业务链路服务，不可用会阻断主流程 |
| **Standard** | 99.5% (两个半九) | 辅助/非实时服务，短时不可用不阻塞主流程 |

例外：`token-service` 作为整个平台鉴权链路的唯一依赖，SLO 提升至 **99.95%**。

---

## 2. 各服务 SLO

| 服务 | Tier | 可用性 SLO | 28d 错误预算 (秒) | 窗口允许不可用时长 | 备注 |
|------|------|-----------|-------------------|-------------------|------|
| `api-gateway` | Core | **99.9%** | 2,419 | ~40 分钟 | 平台唯一入口，限流、路由、鉴权 |
| `token-service` | Core | **99.95%** | 1,210 | ~20 分钟 | JWT 签发、JWKS 端点、整个平台鉴权依赖 |
| `iam-service` | Core | **99.9%** | 2,419 | ~40 分钟 | 用户、角色、权限、认证 |
| `notification-service` | Standard | **99.5%** | 12,096 | ~3.4 小时 | 通知/邮件/短信发送（异步，允许延迟） |
| `audit-service` | Core | **99.9%** | 2,419 | ~40 分钟 | 审计日志写入（合规要求） |
| `config-service` | Core | **99.9%** | 2,419 | ~40 分钟 | 运行时动态配置下发 |
| `file-service` | Standard | **99.5%** | 12,096 | ~3.4 小时 | 文件上传/下载（非实时依赖） |
| `prue_backend` | Standard | **99.5%** | 12,096 | ~3.4 小时 | 流程引擎（SQLite + MySQL 双栈） |

> 注意：防火墙、反向代理、Consul、MySQL、Redis 基础设施本身的可用性不计入上述各服务 SLO，但通过 `ServiceDown` 告警间接覆盖。基础设施层的 SLO 将另行定义。

### 2.1 错误预算计算

以 99.9% (Core) 为例：

```
Error Budget = (1 - 0.999) * 2,419,200 = 2,419.2 秒
```

以 99.95% (token-service) 为例：

```
Error Budget = (1 - 0.9995) * 2,419,200 = 1,209.6 秒
```

---

## 3. SLI 详细定义

每个 SLO 由一个或多个 SLI (Service Level Indicator) 支撑。下表列出本平台的 SLI 标准定义。

### 3.1 Availability (可用性)

**定义**：服务在测量窗口内成功处理请求的比例（排除 5xx 服务端错误）。

**SLI 表达式**：

```promql
sum(rate(http_requests_total{status!~"5.."}[5m]))
  /
sum(rate(http_requests_total[5m]))
```

**说明**：
- `http_requests_total` — 由各服务 `/metrics` 端点暴露的 counter
- `status!~"5.."` — 排除所有 5xx 响应（视为不可用）
- 4xx 响应 (如 401/403/404/429) 不计入不可用——它们是客户端错误或预期的限流行为
- 服务完全宕机 (`up == 0`) 时，`http_requests_total` 无数据点，由 `ServiceDown` 告警单独覆盖

**时间窗口**：Prometheus `rate()` 使用 5 分钟滚动窗口计算瞬时可用率；SLO 合规判定基于 28 天聚合。

### 3.2 Latency (延迟)

**定义**：服务 HTTP 请求的 P99 响应时间。

**SLI 表达式**：

```promql
histogram_quantile(0.99,
  sum by (le, service) (
    rate(http_request_duration_seconds_bucket[5m])
  )
)
```

**说明**：
- `http_request_duration_seconds_bucket` — 由各服务暴露的 histogram
- `histogram_quantile(0.99, ...)` — 计算 P99 分位数
- `sum by (le, service)` — 按服务聚合所有 instance 的 bucket

**阈值**：

| Tier | P99 Latency 阈值 |
|------|-----------------|
| Core | < 500ms |
| Standard | < 1000ms |

### 3.3 Capacity — Outbox Backlog (出箱积压)

**定义**：Transactional outbox 中待发布事件数量。

**SLI 表达式**：

```promql
outbox_pending_total
```

**阈值**：> 1000 条积压视为降级。触发 `OutboxBacklog` 告警（warning）。

### 3.4 Capacity — Rate Limit Exceeded (限流命中)

**定义**：api-gateway 命中速率限制的请求速率。

**SLI 表达式**：

```promql
rate(rate_limit_exceeded_total[5m])
```

**阈值**：> 10 req/s 持续 5 分钟触发 `HighRateLimit` 告警（warning）。

---

## 4. 错误预算策略

### 4.1 预算消耗计算

错误预算消耗率 (burn rate) 定义为：

```promql
# 28d 窗口内不可用时间占比
1 - (
  sum(increase(http_requests_total{status!~"5.."}[28d]))
    /
  sum(increase(http_requests_total[28d]))
)
```

实际业务中用以下 recording rule 持续跟踪：

```promql
# 错误预算消耗比例（0-1）
(
  1 - (
    sum by (service) (rate(http_requests_total{status!~"5.."}[28d]))
      /
    sum by (service) (rate(http_requests_total[28d]))
  )
) / (1 - SLO_target)
```

### 4.2 告警阈值表

| 级别 | Burn Rate | 对应每月预算消耗 | 告警名称 |
|------|-----------|-----------------|---------|
| Warning | >= 0.5 | 50% | `ErrorBudgetBurn_Warning` |
| Critical | >= 0.8 | 80% | `ErrorBudgetBurn_Critical` |

### 4.3 按服务计算示例

以 token-service (99.95%)、28 天、假设 QPS = 100 (稳定)：

- 窗口总请求数 ≈ 100 * 2,419,200 = 241,920,000
- 允许失败请求数 ≈ 241,920,000 * 0.0005 = 120,960
- 即 28 天内最多允许约 12 万个 5xx 响应

---

## 5. 告警规则对应

当前 Prometheus 告警规则定义在 `infrastructure/config/prometheus/alerts.yml`。下表列出告警规则与 SLI 的对应关系：

| 告警名称 | 严重级别 | 对应 SLI | 触发条件 (简要) | for |
|----------|---------|----------|----------------|-----|
| `ServiceDown` | critical | Availability | `up == 0` | 1m |
| `HighErrorRate` | critical | Error Budget | 5xx 错误率 > 1% (5m 窗口) | 5m |
| `HighLatency` | warning | Latency | P99 > 500ms (5m 窗口) | 5m |
| `OutboxBacklog` | warning | Capacity | `outbox_pending_total > 1000` | 5m |
| `HighRateLimit` | warning | Capacity | 限流命中速率 > 10/s (5m 窗口) | 5m |

### 5.1 告警路由

Alertmanager 路由策略 (`infrastructure/config/alertmanager/alertmanager.yml`)：

| 匹配条件 | Receiver | 重复间隔 |
|----------|----------|---------|
| `severity = critical` | `critical` | 1h |
| 其余 | `default` | 4h |

Inhibit 规则：同一 service 触发 critical 时，抑制该 service 的所有 warning / info。

### 5.2 响应动作

| 阶段 | 动作 |
|------|------|
| 告警触发 (Warning) | 值班工程师在 30 分钟内确认、评估趋势 |
| 告警升级 (Critical) | 15 分钟内启动 incident response，通知 on-call |
| Error Budget 耗尽 | 冻结该服务的所有非紧急部署，进入稳定性冻结 (stability freeze) |

---

## 6. 数据来源与查询

### 6.1 Prometheus 作为单一数据源

所有 SLI 数据来源于 Prometheus，通过 Consul service discovery 自动发现所有带 `metrics` tag 的服务。

### 6.2 常用查询速查

```promql
# 各服务当前瞬时可用率
sum by (service) (rate(http_requests_total{status!~"5.."}[5m]))
  /
sum by (service) (rate(http_requests_total[5m]))

# 各服务 P99 时延
histogram_quantile(0.99,
  sum by (le, service) (rate(http_request_duration_seconds_bucket[5m]))
)

# 各容器 /metrics 端点健康状态
up

# 各服务请求速率 (QPS)
sum by (service) (rate(http_requests_total[5m]))

# 各服务 5xx 错误速率
sum by (service) (rate(http_requests_total{status=~"5.."}[5m]))

# 各 route 限流命中数 (最近 1h)
sum by (route) (increase(rate_limit_exceeded_total[1h]))

# 最慢的 5 个 endpoint (按平均延迟)
topk(5,
  sum by (path) (rate(http_request_duration_seconds_sum[5m]))
    /
  sum by (path) (rate(http_request_duration_seconds_count[5m]))
)

# 28 天窗口可用率 (SLO 合规判定)
1 - (
  sum by (service) (increase(http_requests_total{status=~"5.."}[28d]))
    /
  sum by (service) (increase(http_requests_total[28d]))
)

# 错误预算消耗比例
(
  sum by (service) (increase(http_requests_total{status=~"5.."}[28d]))
    /
  sum by (service) (increase(http_requests_total[28d]))
) / scalar(<service_slo_budget>)
```

### 6.3 Grafana Dashboard

预置 `Gateway Overview` 看板 (`infrastructure/config/grafana/dashboards/gateway.json`) 覆盖：

- 按服务的请求速率
- 时延 P50/P95
- 错误率 (5xx)
- 限流命中 (按 route)

建议在 Grafana 中新增独立的 **SLO Overview** 看板，展示各服务 28d 可用率趋势及剩余错误预算。

---

## 7. 例外策略

### 7.1 计划内维护窗口 (Planned Maintenance)

以下场景的不可用时间不消耗错误预算：

| 场景 | 条件 |
|------|------|
| 数据库迁移 (schema migration) | 提前 24h 在 on-call 频道公告 |
| 基础设施升级 (Consul / MySQL / Redis 重启) | 提前 24h 公告，记录在 change log |
| Canary / 灰度发布 | 仅影响 `canary` tagged instance，主集群不受影响 |
| 配置热重载 (`POST /__admin/reload`) | 零停机，不消耗预算 |

维护窗口期间，对应服务的 `ServiceDown` 和 `HighErrorRate` 告警可手动 silence (通过 Alertmanager UI 或 `amtool`)，silence 时长不得超过公告窗口。

### 7.2 4xx 排除

4xx HTTP 状态码 (400-499) 不计入错误预算。原因：

- `401/403` — 鉴权/授权失败，属于正常业务拒绝
- `404` — 客户端请求不存在的资源
- `422` — 请求参数校验失败
- `429` — api-gateway 限流，属于预期保护机制

Prometheus 查询中已通过 `status!~"5.."` 自动排除 4xx。如需单独监控业务错误趋势，可使用独立的 4xx 监控面板，不作为 SLO 依据。

### 7.3 依赖故障传递

当上游依赖（如 MySQL 不可用）导致下游报 5xx 时，该不可用时间计入下游服务的错误预算。各服务应在 `/health` 端点暴露依赖状态，便于故障定位，但不豁免 SLO。

### 7.4 新上线服务宽限期

新服务上线后提供 **14 天宽限期**，期间 SLO 仅记录、不触发 escalation。宽限期结束后正式纳入 on-call 流程。

---

## 参考

| 文档 | 路径 |
|------|------|
| Prometheus 告警规则 | `infrastructure/config/prometheus/alerts.yml` |
| Alertmanager 配置 | `infrastructure/config/alertmanager/alertmanager.yml` |
| Prometheus 采集配置 | `infrastructure/config/prometheus/prometheus.yml` |
| 可观测性指南 | `docs-site/guides/observability.md` |
| Grafana Dashboard | `infrastructure/config/grafana/dashboards/gateway.json` |
| On-call 手册 | `docs-site/runbooks/oncall.md` |
| 故障排查指南 | `docs-site/runbooks/troubleshooting.md` |
