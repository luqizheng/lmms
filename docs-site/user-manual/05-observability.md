# 05. 可观测性

> 看链路、看指标、看日志。

---

## 5.1 技术栈

```
┌─────────────────────────────────────────────────┐
│  服务（api-gateway + 4 个 OTel 已开启的服务）    │
│   ↓ OTLP gRPC :4317                             │
├─────────────────────────────────────────────────┤
│  otel-collector（批量 / resource 增强）          │
│   ↓                              ↓              │
│  Jaeger（trace UI）:16686    Prometheus :8889    │
│   :14250（gRPC 接收）         :9090（scrape）    │
└─────────────────────────────────────────────────┘
       ↑                                 ↑
       │                                 │
  http://localhost:16686/t/...   http://localhost:9090
```

**已开启 OTel 的服务**（4 个）：api-gateway、token-service、iam-service、notification-service。

---

## 5.2 启动可观测性

```bash
# 起基础设施 + 业务服务 + 可观测性（observability 已 include 进主 compose）
docker compose -f infrastructure/docker-compose.yml up -d

# 打开 UI
#    Prometheus  :9090      — 指标查询
#    Grafana     :3001      — 仪表盘（admin/admin）
#    Jaeger      :16686     — 链路 UI
#    OTel collector OTLP gRPC :4317（内部）
```

---

## 5.3 给新服务接 OTel

1. `Cargo.toml` 加依赖：

   ```toml
   common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }
   common-telemetry-axum = { path = "../../libs/common-telemetry-axum", features = ["otlp"] }
   ```

2. `main.rs` 初始化：

   ```rust
   common_telemetry::init_tracing("service-name");
   ```

3. Axum 路由挂中间件：

   ```rust
   .route_layer(axum_mw::from_fn(common_telemetry_axum::trace_request_middleware))
   ```

4. `docker-compose.yml` 加环境变量：

   ```yaml
   environment:
     - OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317
   ```

完整细节见 [`docs-site/guides/otel-tracing.md`](../guides/otel-tracing.md)。

---

## 5.4 按 trace ID 查链路

```bash
# 1. 抓响应头里的 X-Trace-Id
curl -i http://localhost:3000/v1/iam/health | grep X-Trace-Id
# X-Trace-Id: 0af7651916cd43dd8448eb211c80319c

# 2. 在 Jaeger 里打开
#    http://localhost:16686/trace/0af7651916cd43dd8448eb211c80319c
```

也可在终端脚本里组合用：

```bash
TRACE_ID=$(curl -s -i http://localhost:3000/v1/iam/health \
  | awk '/X-Trace-Id:/ {print $2}' | tr -d '\r')

xdg-open "http://localhost:16686/trace/$TRACE_ID"      # Linux
open "http://localhost:16686/trace/$TRACE_ID"          # macOS

# 同时查相关日志
docker logs api-gateway 2>&1 | grep "$TRACE_ID"
```

---

## 5.5 PromQL 常用查询

```promql
# 每服务请求速率
sum by (service) (rate(http_requests_total[1m]))

# 错误率（5xx）
sum by (service) (rate(http_requests_total{status=~"5.."}[5m])) \
  / sum by (service) (rate(http_requests_total[5m]))

# 限流命中（按路由）
sum by (route) (rate(gateway_rate_limited_total[1m]))

# 服务侧限流命中
sum by (service, route) (rate(service_rate_limited_total[1m]))
```

Grafana → **Gateway Overview** 仪表盘已内置前几条；服务侧限流面板会在更多服务接入后补上。

---

## 5.6 日志

- 容器日志：`docker logs -f --tail 200 <container>`
- json 行结构：`ts level service trace_id span_id msg ...`
- 按 trace 过滤：`docker logs <svc> 2>&1 | grep <TRACE_ID>`

详细日志约定见 [`docs/onboarding/07-日志与排查Bug.md`](../onboarding/07-日志与排查Bug.md)。

---

## 5.7 健康 / 就绪端点

每个服务至少暴露：

- `GET /health` —— 进程活着（用于 k8s liveness）
- `GET /metrics` —— Prometheus 抓取
- `GET /ready`（部分服务）—— 依赖就绪（用于 k8s readiness）

> `/health` 与 `/metrics` 是 [02 §2.4 8 项标准](./02-architecture.md#24-8-项服务标准adr-002) 强制项。
