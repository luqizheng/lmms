# 集中式日志收集方案

> **状态**：草案 · 待运维补充
> **关联**：ai-context.md §4.2 Onboarding/运维待推进项
> **适用范围**：coder-micro-services 平台所有服务的日志收集、存储、查询

---

## 0. 一句话

> 当前所有 Rust 服务输出 JSON 格式日志到 stdout，由 `docker logs` 收集。
> 本文档给出三种集中式日志收集方案（ELK / Grafana+Loki / 阿里云 SLS）的架构、配置与接入指南。

---

## 1. 当前日志架构

### 1.1 现状

```
服务进程 → stdout (JSON 格式)
              ↓ docker logs
         本地文件 / 容器日志存储
```

- **日志格式**：`tracing` crate 的 json formatter
- **标准字段**：`ts`（时间戳）、`level`（ERROR/WARN/INFO/DEBUG/TRACE）、`service`（服务名）、`trace_id`、`span_id`、`msg`
- **收集方式**：`docker logs` 或直接查看容器终端输出
- **问题**：无法跨容器关联、无法长期存储、无法聚合查询

### 1.2 标准字段定义

每条日志应包含以下字段：

```json
{
  "ts": "2026-07-13T10:30:00.000Z",
  "level": "ERROR",
  "service": "iam-service",
  "trace_id": "0af7651916cd43dd8448eb211c80319c",
  "span_id": "4bf92f3577b34da6",
  "msg": "Database connection pool exhausted",
  "error": {
    "message": "connection refused",
    "stack": "..."
  },
  "tenant_id": "t-001",
  "user_id": "u-123"
}
```

---

## 2. 方案 A：ELK Stack（Elasticsearch + Logstash + Kibana）

### 2.1 架构

```
服务容器 stdout
      ↓ (Fluentd/Filebeat sidecar 或 Logstash 直接收集)
Elasticsearch 集群 (存储 + 全文索引)
      ↓
Kibana (Web UI 查询 + 可视化)
```

### 2.2 Docker Compose 接入

```yaml
# infrastructure/docker-compose-logging.yml
version: '3.8'

services:
  elasticsearch:
    image: elasticsearch:8.11.0
    environment:
      - discovery.type=single-node
      - xpack.security.enabled=false
      - "ES_JAVA_OPTS=-Xms512m -Xmx512m"
    ports:
      - "9200:9200"
    volumes:
      - es-data:/usr/share/elasticsearch/data
    networks:
      - superpowers

  logstash:
    image: logstash:8.11.0
    volumes:
      - ./config/logstash/pipeline:/usr/share/logstash/pipeline
    depends_on:
      - elasticsearch
    networks:
      - superpowers

  kibana:
    image: kibana:8.11.0
    environment:
      - ELASTICSEARCH_HOSTS=http://elasticsearch:9200
    ports:
      - "5601:5601"
    depends_on:
      - elasticsearch
    networks:
      - superpowers

  # Fluentd sidecar 收集本容器日志
  fluentd:
    image: fluent/fluentd:v1.16
    volumes:
      - ./config/fluentd/fluent.conf:/fluentd/etc/fluent.conf
    depends_on:
      - logstash

volumes:
  es-data:

networks:
  superpowers:
    external: true
```

### 2.3 Fluentd 配置

```conf
# infrastructure/config/fluentd/fluent.conf
<source>
  @type tail
  @id input_tail_container_logs
  path /var/lib/docker/containers/*/*.log
  pos_file /var/log/fluentd-docker-pos.conf
  tag docker.container.*
  <parse>
    @type json
    time_type string
    time_format "%Y-%m-%dT%H:%M:%S.%NZ"
  </parse>
</source>

<match docker.container.**>
  @type elasticsearch
  host elasticsearch
  port 9200
  logstash_format true
  logstash_prefix docker
  include_tag_key true
  <buffer>
    @type file
    path /var/log/fluentd/buffer/els
    flush_mode interval
    flush_interval 10s
  </buffer>
</match>
```

### 2.4 Kibana 查询示例

```
# 查找特定 trace_id 的所有日志
trace_id: "0af7651916cd43dd8448eb211c80319c"

# 查找某服务错误
service: "iam-service" AND level: "ERROR"

# 查找某时间段
@timestamp:[2026-07-01 TO 2026-07-13]

# 某租户的所有错误
tenant_id: "t-001" AND level: "ERROR"
```

---

## 3. 方案 B：Grafana + Loki（轻量级推荐）

### 3.1 架构

```
服务容器 stdout
      ↓ (Promtail sidecar 或 Promtail 直接收集)
Loki (日志存储，类似 Prometheus 的时序DB)
      ↓
Grafana (统一 UI，同时查 metrics + logs)
```

### 3.2 Docker Compose 接入

```yaml
# infrastructure/docker-compose-loki.yml
services:
  loki:
    image: grafana/loki:2.9.0
    ports:
      - "3100:3100"
    volumes:
      - ./config/loki/loki.yaml:/etc/loki/loki.yaml
    command: -config.file=/etc/loki/loki.yaml
    networks:
      - superpowers

  promtail:
    image: grafana/promtail:2.9.0
    volumes:
      - /var/lib/docker/containers:/var/lib/docker/containers:ro
      - ./config/promtail/promtail.yaml:/etc/promtail/promtail.yaml
    command: -config.file=/etc/promtail/promtail.yaml
    networks:
      - superpowers
    depends_on:
      - loki
```

### 3.3 Loki 配置

```yaml
# infrastructure/config/loki/loki.yaml
auth_enabled: false

server:
  http_listen_port: 3100

ingester:
  lifecycler:
    address: 127.0.0.1
  chunk_idle_period: 15m
  max_transfer_retries: 0

schema_config:
  configs:
    - from: 2026-01-01
      store: boltdb
      object_store: filesystem
      schema: v11
      index:
        prefix: index_
        period: 168h

storage_config:
  boltdb:
    directory: /data/index
  filesystem:
    directory: /data/chunks

limits:
  reject_old_samples: true
  reject_old_samples_max_age: 168h
```

### 3.4 Promtail 配置

```yaml
# infrastructure/config/promtail/promtail.yaml
server:
  http_listen_port: 9080
  grpc_listen_port: 0

positions:
  filename: /var/log/promtail/positions.yaml

clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: docker
    static_configs:
      - targets:
          - localhost
        labels:
          job: containerlogs
          __path__: /var/lib/docker/containers/*/*-json.log
    pipeline_stages:
      - json:
          expression: "^(.{19}).level.\"service\".msg.*"
          action: parse
      - json:
          expression: "^{.*}$"
          action: parse
```

### 3.5 Grafana Loki 查询示例（LogQL）

```logql
# 查找特定 trace_id
{service="iam-service"} |= "0af7651916cd43dd8448eb211c80319c"

# 某服务错误率随时间变化
sum by (level) (count_over_time({service="api-gateway"} | json [5m]))

# 某时间段内所有错误
{service=~".*"} | json | level="ERROR" | __error__!="JSONParserErr"

# 按服务聚合错误数
sum(count_over_time({service=~".*"} | json | level="ERROR" [5m])) by (service)
```

---

## 4. 方案 C：阿里云日志服务（SLS）

### 4.1 架构

```
服务容器 stdout
      ↓ (Aliyun Logtail)
阿里云 SLS (日志服务)
      ↓
LogSearch / Analytics / Alerts
```

### 4.2 Rust 日志接入阿里云 SLS

添加依赖：

```toml
# Cargo.toml
[dependencies]
alibaba-logging-sdk = "0.9"
```

配置：

```rust
use alibaba_logging_sdk::Logger;

let logger = Logger::new()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .project("your-project")
    .logstore("coder-micro-services")
    .access_key_id("your-access-key-id")
    .access_key_secret("your-access-key-secret")
    .build();

logger.log(
    alibaba_logging_sdk::Level::ERROR,
    "iam-service",
    "Database connection exhausted",
    &[("trace_id", trace_id), ("tenant_id", tenant_id)],
);
```

### 4.3 Docker 环境变量模式（推荐）

```yaml
# 阿里云 SLS Logtail 作为 sidecar
services:
  logtail:
    image: logtail.docker/logtail:latest
    environment:
      - ALIYUNSLS_ENDPOINT=cn-hangzhou.log.aliyuncs.com
      - ALIYUNSLS_PROJECT=coder-micro-services-prod
      - ALIYUNSLS_LOGSTORE=app-logs
      - ALIYUNSLS_ACCESS_KEY_ID=${ALIYUN_ACCESS_KEY_ID}
      - ALIYUNSLS_ACCESS_KEY_SECRET=${ALIYUN_ACCESS_KEY_SECRET}
    volumes:
      - /var/lib/docker/containers:/var/lib/docker/containers:ro
```

---

## 5. 日志级别规范

| 级别 | 使用场景 | 示例 |
|------|----------|------|
| ERROR | 错误导致请求失败 | DB 连接失败、panic、5xx |
| WARN | 异常但不阻断 | 重试、限流命中、降级 |
| INFO | 重要业务事件 | 登录成功、配置变更、部署 |
| DEBUG | 调试信息 | 请求参数、中间状态 |
| TRACE | 详细追踪 | SQL 语句、函数入口 |

**原则**：
- 生产默认 INFO 以上
- 本地开发 DEBUG
- 错误必须带 context（trace_id、tenant_id、user_id）

---

## 6. 查询示例与调试流程

### 6.1 按 trace_id 串联日志

```bash
# 1. 从 Jaeger 或响应头拿到 trace_id
TRACE_ID="0af7651916cd43dd8448eb211c80319c"

# 2. ELK 查询
trace_id: "${TRACE_ID}"

# Loki LogQL
{service=~".*"} |= "${TRACE_ID}"

# 阿里云 SLS
trace_id: "${TRACE_ID}"

# 3. 输出该 trace 经过的所有服务的日志（按时间排序）
```

### 6.2 错误率告警

```yaml
# Prometheus alerting rules (for metrics-based alerts)
groups:
  - name: logging-alerts
    rules:
      - alert: HighErrorRate
        expr: |
          sum by (service) (rate(log_messages_total{level="ERROR"}[5m]))
          / sum by (service) (rate(log_messages_total[5m])) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "服务 {{ $labels.service }} 错误率超过 5%"
```

---

## 7. 告警设置

### 7.1 Loki + Grafana 告警

```yaml
# Grafana alerting rule
{
  "name": "High Error Rate",
  "condition": "B",
  "data": [
    {
      "expr": "sum(rate({service=~\".*\"} | json | level=\"ERROR\" [5m])) by (service)",
      "refId": "A"
    },
    {
      "expr": "sum(rate({service=~\".*\"} [5m])) by (service) * 0.05",
      "refId": "B"
    }
  ]
}
```

---

## 8. 关联

- `docs-site/runbooks/troubleshooting.md` — 故障排查
- `docs-site/guides/otel-tracing.md` — 链路追踪
- `docs-site/guides/observability.md` — 可观测性
- `infrastructure/docker-compose.yml` — 基础设施配置

---

> **最后更新**：2026-07-13
