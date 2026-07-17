# 可观测性指南（Observability Guide）

> 最近更新：2026-06-25

本文档涵盖：

1. 技术栈总览（Prometheus + Grafana + OTel Collector + Jaeger）
2. 启动可观测性 overlay
3. Metrics — 暴露内容与查询方式
4. Traces — 端到端分布式链路追踪
5. 跨服务 trace 串联
6. 新服务接入可观测性
7. 常见问题排查

仅看 OTel 配置细节，请参阅 [`docs/guides/otel-tracing.md`](otel-tracing.md)。
灰度发布请参阅 [`docs/guides/canary-rollout.md`](canary-rollout.md)。

---

## 1. 技术栈总览

```
┌─────────────────────────────────────────────────────────┐
│  Services (Rust)                                         │
│   ↓ /metrics (Prometheus text format)                   │
│   ↓ OTLP gRPC :4317 (启用 OTEL 时)                       │
├─────────────────────────────────────────────────────────┤
│  Prometheus ◀── scrape :9090 ──┐                        │
│  OTel collector ──── Jaeger    │                        │
│       │ :4317 → :8889 (RED metrics) :14250 → :16686 UI  │
│       │                                                  │
│       └─ Grafana :3001 ← reads Prometheus               │
└─────────────────────────────────────────────────────────┘
       ↓                       ↓
   metrics 查询              traces UI
```

| 组件 | 镜像 | 端口 | 用途 |
|------|------|------|------|
| **Prometheus** | `prom/prometheus:v2.51.0` | 9090 | 指标采集 + 告警 |
| **Grafana** | `grafana/grafana:10.4.0` | 3001 | 看板（admin / admin） |
| **Jaeger** | `jaegertracing/all-in-one:1.55` | 16686 (UI), 14250 (gRPC) | 分布式链路 UI |
| **OTel Collector** | `otel/opentelemetry-collector-contrib:0.96.0` | 4317 (gRPC), 4318 (HTTP), 8889 (metrics) | OTLP 接收器 + 导出器 |

全部定义在 `infrastructure/docker-compose-observability.yml`（独立 overlay）。

---

## 2. 启动可观测性

### 2.1 一键启动

```bash
# 启动主基础设施 + 可观测性（observability 已 include 进主 compose）
docker compose -f infrastructure/docker-compose.yml up -d
```

启动后会拉起 Prometheus、Grafana、Jaeger、OTel collector、consul-template 五个可观测性容器。

### 2.2 访问地址

| 服务 | URL | 凭证 |
|------|-----|------|
| Prometheus | http://localhost:9090 | — |
| Grafana | http://localhost:3001 | admin / admin |
| Jaeger UI | http://localhost:16686 | — |
| OTel collector OTLP gRPC | localhost:4317 | — |

### 2.3 验证 Prometheus 在采集

```bash
# 列出所有活跃 scrape target
curl -s http://localhost:9090/api/v1/targets | \
  jq '.data.activeTargets[] | {service: .labels.service, health: .health}'

# 应包含：audit-service, config-service, file-service, iam-service,
#          notification-service, prue_backend, token-service, api-gateway
```

### 2.4 验证 Jaeger 在收 span

```bash
# Jaeger 已知的服务列表
curl -s http://localhost:16686/api/services
# 流量起来后应能看到：api-gateway, token-service, iam-service, notification-service
# （目前 4 个启用 OTel 的服务）
```

---

## 3. Metrics

### 3.1 暴露内容

每个服务都有 `/metrics` 端点，返回 Prometheus 文本格式。内置指标：

| 指标 | 类型 | 标签 | 来源 |
|------|------|------|------|
| `http_requests_total` | counter | method, path, status | 服务代码（可能缺省） |
| `http_request_duration_seconds_bucket` | histogram | method, path | 服务代码 |
| `gateway_rate_limited_total` | counter | route | api-gateway 限流中间件 |
| 进程 / 运行时指标 | 各类 | — | `prometheus` crate 默认 |

加上由 OTel collector 从 span 自动派生的 RED 指标。

### 3.2 预置 Grafana 看板

`infrastructure/config/grafana/dashboards/gateway.json` 提供 **Gateway Overview** 看板，包括：

- **按服务的请求速率** — `sum by (service) (rate(http_requests_total[1m]))`
- **时延 p50 / p95** — `histogram_quantile(...)`
- **错误率（5xx）** — `sum by (service) (rate(http_requests_total{status=~"5.."}[5m])) / sum by (service) (rate(http_requests_total[5m]))`
- **限流命中（按 route）** — `sum by (route) (rate(gateway_rate_limited_total[1m]))`

通过 provisioning 在 Grafana 启动时自动加载。

### 3.3 常用 PromQL

```promql
# 各服务的请求速率（最近 5 分钟）
sum by (service) (rate(http_requests_total[5m]))

# 各服务的 p99 时延
histogram_quantile(0.99, sum by (le, service) (rate(http_request_duration_seconds_bucket[5m])))

# 各 route 的限流命中数（最近 1 小时）
sum by (route) (increase(gateway_rate_limited_total[1h]))

# 最慢的 5 个 endpoint
topk(5, sum by (path) (rate(http_request_duration_seconds_sum[5m])) / sum by (path) (rate(http_request_duration_seconds_count[5m])))

# 错误预算消耗速度（5xx）
sum by (service) (rate(http_requests_total{status=~"5.."}[1h])) / sum by (service) (rate(http_requests_total[1h]))
```

---

## 4. Traces (OpenTelemetry)

### 4.1 架构

```
api-gateway    ──┐
token-service  ──┤── otel-collector ── Jaeger
iam-service    ──┤                     (UI :16686)
notification   ──┘

每个服务：
- 使用 common-telemetry crate（init_tracing）
- 设置了 OTEL_EXPORTER_OTLP_ENDPOINT 时，OTel layer 才会被激活
- 通过 OTLP gRPC :4317 导出 span
```

### 4.2 哪些事件会被 trace

每一次 HTTP 请求都会产生：

```
http_request [根 span]
├─ http.method, http.path, http.status_code, duration_ms
├─ otel.kind = "server"
└─ 子 span：
   - auth_middleware（api-gateway）
   - rate_limit_middleware（api-gateway）
   - proxy_handler → 向下游服务发起的 HTTP 调用（api-gateway）
   - 下游服务自己的 http_request span（在下游也启用 OTel 的前提下）
```

### 4.3 查一条 trace

```bash
# 1. 从响应中拿到 X-Trace-Id
curl -i http://localhost:3000/v1/iam/health | grep X-Trace-Id
# X-Trace-Id: 0af7651916cd43dd8448eb211c80319c

# 2. 直接打开 Jaeger UI 查看
xdg-open "http://localhost:16686/trace/0af7651916cd43dd8448eb211c80319c"
```

在 Jaeger UI：

- Service 下拉：`api-gateway`（或 `token-service` / `iam-service` / `notification-service`）
- Operation：`http_request`
- 点击 "Find Traces"

### 4.4 Span 属性

span 上会携带：

| 属性 | 示例 | 来源 |
|------|------|------|
| `http.method` | `POST` | tracing middleware |
| `http.path` | `/v1/iam/users` | tracing middleware |
| `http.status_code` | `200`、`429`、`500` | tracing middleware |
| `duration_ms` | `42` | tracing middleware |
| `otel.kind` | `server` | tracing middleware |
| `service.name` | `api-gateway` | OTel resource |
| `deployment.environment` | `dev` | OTel collector resource processor |

业务代码里可以通过 `tracing::info_span!(..., custom_field = value)` 添加自定义属性。

---

## 5. 跨服务 trace 串联

### 5.1 传播机制

`api-gateway` 转发到下游服务时：

1. `proxy_handler` 从当前 span 拿到 context
2. 调用 OTel 的 text map propagator（默认 W3C TraceContext）
3. 把 `traceparent` header 注入到出站请求里
4. 下游服务的 tracing middleware 从入站请求里抽 `traceparent`
5. 调用 `span.set_parent(cx)` 把 span 串起来

最终：**Jaeger 中得到一条覆盖 4 个服务的完整 trace**。

### 5.2 Trace tree 示例

```
[trace 0af765...] api-gateway POST /v1/auth/login
├─ http_request [api-gateway]
│  ├─ auth_middleware
│  ├─ rate_limit_middleware (pass)
│  └─ proxy_handler
│     └─ (HTTP to iam-service:6002)
│
├─ http_request [iam-service]
│  ├─ validate_user
│  ├─ bcrypt_verify
│  └─ mint_token
│     └─ (HTTP to token-service:6001)
│
└─ http_request [token-service]
   ├─ load_keys
   └─ sign_jwt
```

### 5.3 已启用 OTel 的服务（截至 2026 年 6 月）

| 服务 | 状态 |
|------|------|
| api-gateway | ✅ 已启用 |
| token-service | ✅ 已启用 |
| iam-service | ✅ 已启用 |
| notification-service | ✅ 已启用 |
| prue_backend | — 暂未启用 |
| audit-service | — 暂未启用 |
| config-service | — 暂未启用 |
| file-service | — 暂未启用 |

如何给新服务启用，见 § 6。

### 5.4 响应中的 trace_id

所有启用了 OTel 的服务的 HTTP 响应都会带 `X-Trace-Id` header：

```http
HTTP/1.1 200 OK
X-Trace-Id: 0af7651916cd43dd8448eb211c80319c
Content-Type: application/json
```

即使服务没有 OTel subscriber（即 tracing 关闭），中间件也会用一个随机 128-bit ID 兜底，保证客户端能做关联。

---

## 6. 新服务接入可观测性

### 6.1 Metrics（所有服务必备）

每个服务都已经通过 `prometheus` crate 暴露了 `/metrics`（P1.3 baseline）。要加自定义指标：

```rust
use prometheus::{IntCounterVec, register_int_counter_vec};

pub static MY_METRIC: once_cell::sync::Lazy<IntCounterVec> =
    once_cell::sync::Lazy::new(|| {
        register_int_counter_vec!(
            "my_service_metric_total",
            "Description",
            &["label_a", "label_b"]
        ).unwrap()
    });

// 在 handler 里：
MY_METRIC.with_label_values(&["x", "y"]).inc();
```

### 6.2 Traces（按服务可选启用）

给一个不在已启用列表里的服务接入 OTel：

**Step 1**：更新 `Cargo.toml`：

```toml
common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }
opentelemetry = "0.24"
tracing-opentelemetry = "0.25"
rand.workspace = true
```

**Step 2**：添加 `common-telemetry-axum` 依赖（Cargo.toml）：

```toml
common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }
common-telemetry-axum = { path = "../../libs/common-telemetry-axum", features = ["otlp"] }
```

**Step 3**：在 `src/main.rs` 中初始化 tracing 并挂 middleware：

```rust
common_telemetry::init_tracing("<your-service>");
// ...
.layer(axum::middleware::from_fn(common_telemetry_axum::trace_request_middleware))
```

**Step 4**：在 `infrastructure/docker-compose.yml` 中：

```yaml
<your-service>:
  environment:
    - OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
```

**Step 7**：验证：

```bash
docker compose up -d
curl -i http://localhost:<port>/health | grep X-Trace-Id
# 打开 http://localhost:16686 查看该 trace
```

### 6.3 采样（高流量场景）

默认：100% 采样（每个请求都 trace）。高流量场景启用采样：

```bash
# 采样 10%
OTEL_TRACES_SAMPLER=TraceIdRatio
OTEL_TRACES_SAMPLER_ARG=0.1
```

可在 `common-telemetry` 里加上配置开关，使其按环境可调。

---

## 7. 常见问题

### 7.1 “Jaeger 中没有任何 trace”

1. 检查 `OTEL_EXPORTER_OTLP_ENDPOINT` 是否设置：
   ```bash
   docker exec api-gateway env | grep OTEL
   ```
2. 检查 collector 是否可达：
   ```bash
   docker exec api-gateway curl -v http://otel-collector:4317
   ```
3. 检查 collector 日志：
   ```bash
   docker logs observability-otel-collector | tail -50
   ```
4. 检查 Jaeger 是否真的收到数据：
   ```bash
   curl http://localhost:16686/api/services
   ```

### 7.2 “Prometheus 所有 target 都 DOWN”

- 等待至少 15 秒做第一轮 scrape
- 检查服务在 Consul 中是否带 `metrics` 标签：
  ```bash
  curl http://localhost:8500/v1/catalog/services | jq .
  ```
- 直接访问服务的 `/metrics`：
  ```bash
  curl http://localhost:6002/metrics
  ```

### 7.3 “trace 里没有 span”

- OTel subscriber 没有初始化。检查服务启动时 `OTEL_EXPORTER_OTLP_ENDPOINT` 是否设置
- 检查服务启动日志里是否打印了 `"OTel tracing enabled → ..."`

### 7.4 “内存 / CPU 过高”

调整批处理参数：

```bash
OTEL_BSP_SCHEDULE_DELAY=10000   # 每 10 秒批量上报一次
OTEL_BSP_MAX_BATCH_SIZE=500     # 单次最多 500 个 span
```

可在 `common-telemetry::init_tracing` 中暴露为配置项。

### 7.5 “想在不重启的前提下关掉 trace”

去掉 `OTEL_EXPORTER_OTLP_ENDPOINT` 然后重启服务即可。没有该环境变量时，`common_telemetry` 会回退到仅输出 JSON 日志。

---

## 8. 参考

- `docs/guides/otel-tracing.md` — OTel 详细配置
- `docs/guides/canary-rollout.md` — 灰度发布 runbook
- `docs/user-manual/05-observability.md` — 速查参考
- `infrastructure/config/prometheus/prometheus.yml` — Prometheus 采集配置
- `infrastructure/config/otel/collector.yaml` — OTel collector pipeline
- `infrastructure/config/grafana/dashboards/gateway.json` — 看板 JSON
- `libs/common-telemetry/src/lib.rs` — 源码参考
