# 10. 故障排查

> 先看症状速查表；找不到再按 trace 排查；最后才考虑整体重置。

---

## 10.1 常见错误速查

| 症状 | 可能原因 | 修法 |
|------|----------|------|
| `cargo: linker 'cc' not found` | 本地 toolchain 缺链接器 | 走 `node scripts/run-tests-docker.js <svc>` |
| `api-gateway` blacklist 测试失败 | 本机无 Redis 依赖 | 走 docker（已带 test Redis） |
| `utoipa::to_yaml()` 找不到 | 缺 `yaml` feature | 在 Cargo.toml 加 `features = ["axum_extras", "chrono", "yaml"]` |
| `429 Too Many Requests` | 命中限流 | 调 `infrastructure/config/api-gateway/rate-limits.toml` 或退避重试 |
| OpenAPI drift CI 失败 | 手改了 yaml | 跑 `scripts/dump-openapi-specs.sh` 后重提 |
| 服务没注册到 Consul | 容器不在 `superpowers` 网络 | compose 加 `networks: [superpowers]` |
| Jaeger 看不到 trace | `OTEL_EXPORTER_OTLP_ENDPOINT` 未设 | compose / env 加 `http://otel-collector:4317` |
| Swagger 404 | `SWAGGER_ENABLED=false` | 设成 `true` |
| `iam_outbox_events` 表无限增长 | archive cron 每 5 min 跑一次；>24h 的活行会移到 `iam_outbox_events_archive`，30 天后清理。查 `iam-outbox-publisher` 日志看归档计数 | — |

---

## 10.2 整体重置（核弹）

```bash
# 拆掉所有 infra + observability（带 -v 删卷）
docker compose -f infrastructure/docker-compose.yml down -v
docker network rm superpowers 2>/dev/null

# 重新起
docker compose -f infrastructure/docker-compose.yml up -d
```

> 这会**清掉所有本地数据**。生产禁止。

---

## 10.3 按 trace 排查具体请求

```bash
# 1. 发请求，抓 trace ID
TRACE_ID=$(curl -s -i http://localhost:3000/v1/iam/health \
  | awk '/X-Trace-Id:/ {print $2}' | tr -d '\r')

# 2. 在 Jaeger 里看完整链路
xdg-open "http://localhost:16686/trace/$TRACE_ID"      # Linux
open "http://localhost:16686/trace/$TRACE_ID"          # macOS

# 3. 在所有相关容器日志里搜
docker logs api-gateway 2>&1 | grep "$TRACE_ID"
docker logs iam-service 2>&1 | grep "$TRACE_ID"
```

如果响应头没有 `X-Trace-Id`，说明 gateway 没装链路中间件（应已默认开启，见 [05 §5.3](./05-observability.md#53-给新服务接-otel)）。

---

## 10.4 数据库类问题

| 症状 | 怎么查 |
|------|--------|
| 迁移没生效 | 看 `services/<svc>/migrations/` 与 DB 实际表 |
| sqlx 报 "relation does not exist" | 重跑迁移，或确认 `DATABASE_URL` 指向对的库 |
| prue_backend 用 SQLite 测 | 默认 `migrations/sqlite/` 下要有对应版本 |
| 表名漂移（CI 拦截） | 跑 `node scripts/check-db-naming.js` 看规则 |

迁移规范：forward-only，不编辑已发布的迁移，新增 `V{N+1}__*.sql`。

---

## 10.5 服务起不来

```bash
# 1. 看容器日志
docker logs --tail 200 <container>

# 2. 看健康端点（端口看场景）
#    本地 cargo run：默认 :8080（如 iam、token 等）
#    docker compose 内 mesh 服务：内端口 :180xx，需在 sidecar namespace 内
docker exec iam-service-sidecar wget -qO- http://127.0.0.1:18081/health
#    docker compose 内明文服务（file / config）：外端口
curl -i http://localhost:8082/health   # file-service
#    走 api-gateway（任意场景都可用）：
curl -i http://localhost:3000/v1/iam/health

# 3. 进容器交互
docker exec -it <container> sh

# 4. 看进程 / 端口
docker exec <container> ps aux
docker exec <container> netstat -tlnp 2>/dev/null || ss -tlnp
```

常见根因：

- 端口被占
- 配置文件路径错（绝对 vs 相对）
- 依赖服务（MySQL/Redis/Consul）未就绪就启动 → 等健康
- sidecar 没注册到 Consul / mTLS 握手失败 → 看 `<svc>-sidecar` 容器日志
- `ConsulRegistrar` UDP probe fallback 到 hostname，Consul DNS 解析不到 → 手动更新注册地址或 rebuild `libs/consul-registrar`

---

## 10.6 性能问题

```bash
# 看 P99 / 错误率（Grafana）
# 或 PromQL：
sum by (service) (rate(http_requests_total{status=~"5.."}[5m])) \
  / sum by (service) (rate(http_requests_total[5m]))

# 找最慢路由
topk(10,
  histogram_quantile(0.99,
    sum by (route, le) (rate(http_request_duration_seconds_bucket[5m]))
  )
)
```

性能退化先看：

1. 上游依赖（DB / Redis / Consul）是否慢
2. 是否有新部署（关联 commit）
3. 是否有突发流量（看 dashboard）
4. 限流是否被命中（被命中不是 bug，但暴露容量不足）

---

## 10.7 还没解决？

1. 翻 [`docs/onboarding/07-日志与排查Bug.md`](../onboarding/07-日志与排查Bug.md)（中文新手视角）
2. 翻 [`docs-site/ai-context.md`](../ai-context.md) § pitfall 列表（项目踩过的坑）
3. 翻 [`docs-site/decisions/`](../decisions/) ADR（设计意图）
4. 提 issue，附：trace ID、复现步骤、预期 vs 实际
