# OpenTelemetry 链路追踪手册

> 如何启用、观察、调试基于 OTel 技术栈的分布式链路追踪。

## 架构

```
┌─────────────┐  OTLP gRPC :4317   ┌──────────────────┐
│ api-gateway │ ─────────────────► │ otel-collector   │
└─────────────┘                    │  （批处理 /      │
                                   │   resource 加工） │
                                   └────────┬─────────┘
                                            │
                          ┌─────────────────┼─────────────────┐
                          ▼                                   ▼
                  ┌───────────────┐                  ┌────────────────┐
                  │ Jaeger :16686 │                  │ Prometheus     │
                  │   (UI)        │                  │   :8889 (RED   │
                  │   traces UI   │                  │    metrics)    │
                  └───────────────┘                  └────────────────┘
```

## Step 1：启动链路栈

```bash
docker compose -f infrastructure/docker-compose.yml up -d
```

这会拉起：

- `otel-collector`（OTLP 接收：:4317 gRPC / :4318 HTTP）
- `jaeger`（UI: http://localhost:16686）

## Step 2：在某个服务启用 OTel

`common-telemetry` crate 的 OTel layer 通过 `otlp` feature 门控，只有设置了 `OTEL_EXPORTER_OTLP_ENDPOINT` 环境变量时才会激活。

`api-gateway` 已经接好了：

```yaml
# 在 infrastructure/docker-compose.yml 的 api-gateway 段下：
environment:
  OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
```

给其它服务接入：

1. 在依赖中开启 `otlp` feature：
   ```toml
   common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }
   ```
2. 启动时调用 `common_telemetry::init_tracing("service-name")`
3. 设置 `OTEL_EXPORTER_OTLP_ENDPOINT` 环境变量

不设置该环境变量时，服务会回退到 JSON stdout 日志（无任何行为变化）。

## Step 3：验证一条 trace

1. 通过网关发个请求：
   ```bash
   curl -i http://localhost:3000/v1/iam/health
   ```
   查看响应头，应当能看到 `X-Trace-Id: <32-hex>`

2. 打开 Jaeger UI：http://localhost:16686
   - Service：`api-gateway`
   - Operation：`http_request`
   - 点击 "Find Traces"

3. 应当能看到 span 树：
   ```
   http_request
     ├─ auth_middleware
     ├─ rate_limit_middleware
     └─ proxy_handler
         └─ （转发到下游 HTTP，trace_id 自动串联）
   ```

## Step 4：日志 ↔ trace 关联

请求 span 内输出的每条日志都会自动携带 `trace_id`（由 `tracing-opentelemetry` 注入）。按 trace 过滤日志：

```bash
docker logs api-gateway 2>&1 | grep "<trace_id>"
```

## Step 5：跨服务 trace 串联

api-gateway 在转发请求时会把 W3C `traceparent` header 注入到出站请求里。下游服务只要也启用了 OTel，就会自动续上这条 trace。

**已启用 OTel 的服务（2026 年 6 月）：**

- `api-gateway` — 入口；解析入站 traceparent，并向下游注入
- `token-service` — JWT 签发
- `iam-service` — 用户 / 角色 / 权限查询
- `notification-service` — 公告 + WebSocket 推送

```yaml
# gateway → iam-service → token-service 的一条 trace：
api-gateway         POST /v1/auth/login
  └─ http_request
       └─ proxy_handler
            └─ iam-service   POST /v1/iam/auth/login
                 └─ http_request
                      └─ token-service  POST /tokens/access
                           └─ http_request
```

在 Jaeger 中用 `service.api-gateway` 搜索就能看到完整链路。

`docker-compose.yml` 已经给这 4 个服务都设置了 `OTEL_EXPORTER_OTLP_ENDPOINT`，所以只要 `docker compose -f infrastructure/docker-compose.yml up -d` 在跑，它们就会把 span 导到同一个 collector → Jaeger。

## 常见问题

### “启用了 OTEL_EXPORTER_OTLP_ENDPOINT 但 Jaeger 里没有 trace”

1. 确认服务能访问 collector：
   ```bash
   docker exec api-gateway curl -v http://otel-collector:4317
   ```
2. 查看 collector 日志：
   ```bash
   docker logs observability-otel-collector | tail -50
   ```
3. 看 Jaeger 是否真的收到了数据：
   ```bash
   curl http://localhost:16686/api/services
   # 发过请求后应当能看到 api-gateway
   ```

### “服务起来了，但 trace 里没有 service.name”

`init_tracing` 的 `service_name` 参数就是 OTel 上报的 `service.name` resource 属性。确保调用时传入了非空字符串。

### “CPU / 内存过高”

OTel 会批量上报 span，默认批大小 1000、超时 5 秒，先到先发。可覆盖：

```bash
OTEL_BSP_SCHEDULE_DELAY=10000  # 每 10 秒批量上报一次
OTEL_BSP_MAX_BATCH_SIZE=500
```

### “想在不重启的前提下关闭 trace”

不要设置 `OTEL_EXPORTER_OTLP_ENDPOINT`，服务会自动回退到仅写 stdout 的 tracing 模式。

## 采样

默认：100%（所有请求都 trace）。仅采样 10%：

```bash
OTEL_TRACES_SAMPLER=TraceIdRatio
OTEL_TRACES_SAMPLER_ARG=0.1
```

如果想在网关侧做 head-based 采样：在 collector.yaml 的 `tail_sampling` processor 中配置。

## 暂不在范围内

- 从 span 派生 metrics（RED metrics）：由 OTel collector 的 prometheus exporter 在 :8889 暴露
- 日志关联：需要 `tracing-subscriber` 的 json formatter（默认已启用）
- 业务事件的自定义 span 属性（如 `menu_config.etag_conflict`）：在具体代码路径加 `#[instrument]` 或手工 `tracing::info_span!`
