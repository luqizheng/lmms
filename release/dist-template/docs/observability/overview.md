# 可观测性

## Prometheus

指标采集器，收集所有微服务的 OTEL metrics。

- **地址**：http://localhost:9090
- **端口**：9090

常用查询：
```promql
# 服务存活状态
up

# 请求速率
rate(http_requests_total[5m])

# 错误率
rate(http_requests_total{status=~"5.."}[5m])

# P99 延迟
histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))
```

## Grafana

可视化仪表盘，连接 Prometheus 作为数据源。

- **地址**：http://localhost:3001
- **账号**：admin / admin（首次登录需修改密码）

预置仪表盘：
- **Gateway Overview**：API 网关 QPS/延迟/错误率
- **Service Performance**：各微服务的请求量和错误分布

## Jaeger

分布式追踪，接收 OTLP traces。

- **地址**：http://localhost:16686
- **端口**：16686

支持：
- 按 trace ID 查询完整调用链
- 查看服务间依赖关系
- 分析慢请求

## OpenTelemetry Collector

接收各服务的 OTLP 数据（traces → Jaeger，metrics → Prometheus）。

- **OTLP gRPC**：4317
- **OTLP HTTP**：4318
- **Prometheus exporter**：8889

## Consul-template

自动监控 Consul 服务变化，渲染 nginx 配置文件（用于 canary 灰度）。

## 日志聚合

各服务的 RUST_LOG 统一输出到 Docker stdout，通过：

```bash
# 查看实时日志
make logs

# 导出全部日志
docker compose logs > all_logs.txt
```
