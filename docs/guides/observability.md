# Observability Guide

> Last updated: 2026-06-25

This document covers:
1. Stack overview (Prometheus + Grafana + OTel Collector + Jaeger)
2. Bringing up the observability overlay
3. Metrics — what's exposed and how to query
4. Traces — distributed tracing end-to-end
5. Cross-service trace chains
6. Adding observability to a new service
7. Common issues

For OTel-only configuration details, see `docs/guides/otel-tracing.md`.
For canary rollout, see `docs/guides/canary-rollout.md`.

---

## 1. Stack overview

```
┌─────────────────────────────────────────────────────────┐
│  Services (Rust)                                         │
│   ↓ /metrics (Prometheus text format)                   │
│   ↓ OTLP gRPC :4317 (when OTEL enabled)                 │
├─────────────────────────────────────────────────────────┤
│  Prometheus ◀── scrape :9090 ──┐                        │
│  OTel collector ──── Jaeger    │                        │
│       │ :4317 → :8889 (RED metrics) :14250 → :16686 UI  │
│       │                                                  │
│       └─ Grafana :3001 ← reads Prometheus               │
└─────────────────────────────────────────────────────────┘
       ↓                       ↓
   metrics queries         traces UI
```

| Component | Image | Port | Purpose |
|-----------|-------|------|---------|
| **Prometheus** | `prom/prometheus:v2.51.0` | 9090 | Metrics scraping + alerting |
| **Grafana** | `grafana/grafana:10.4.0` | 3001 | Dashboards (admin/admin) |
| **Jaeger** | `jaegertracing/all-in-one:1.55` | 16686 (UI), 14250 (gRPC) | Distributed tracing UI |
| **OTel Collector** | `otel/opentelemetry-collector-contrib:0.96.0` | 4317 (gRPC), 4318 (HTTP), 8889 (metrics) | OTLP receiver + exporter |

All defined in `infrastructure/docker-compose-observability.yml` (standalone overlay).

---

## 2. Bring up observability

### 2.1 One-command setup

```bash
# 1. Start main infrastructure
docker compose -f infrastructure/docker-compose.yml up -d

# 2. Start observability overlay
scripts/start-observability.sh
```

The script prints the URLs and verifies the superpowers network exists.

### 2.2 Access URLs

| Service | URL | Credentials |
|---------|-----|-------------|
| Prometheus | http://localhost:9090 | — |
| Grafana | http://localhost:3001 | admin / admin |
| Jaeger UI | http://localhost:16686 | — |
| OTel collector OTLP gRPC | localhost:4317 | — |

### 2.3 Verify Prometheus is scraping

```bash
# List active scrape targets
curl -s http://localhost:9090/api/v1/targets | \
  jq '.data.activeTargets[] | {service: .labels.service, health: .health}'

# Should list: audit-service, config-service, file-service, iam-service,
#              notification-service, prue_backend, token-service, api-gateway
```

### 2.4 Verify Jaeger is receiving spans

```bash
# List services Jaeger knows about
curl -s http://localhost:16686/api/services
# Should list: api-gateway, token-service, iam-service, notification-service
# (4 OTel-enabled services, after some traffic flows)
```

---

## 3. Metrics

### 3.1 What's exposed

Every service has `/metrics` endpoint returning Prometheus text format. Built-in metrics:

| Metric | Type | Labels | Source |
|--------|------|--------|--------|
| `http_requests_total` | counter | method, path, status | Service code (or absent) |
| `http_request_duration_seconds_bucket` | histogram | method, path | Service code |
| `gateway_rate_limited_total` | counter | route | api-gateway rate-limit middleware |
| Process / runtime metrics | various | — | `prometheus` crate default |

Plus RED metrics auto-generated from OTel spans by the collector.

### 3.2 Pre-built Grafana dashboard

`infrastructure/config/grafana/dashboards/gateway.json` ships a **Gateway Overview** dashboard with:

- **Request Rate by Service** — `sum by (service) (rate(http_requests_total[1m]))`
- **Latency p50 / p95** — `histogram_quantile(...)`
- **Error Rate (5xx)** — `sum by (service) (rate(http_requests_total{status=~"5.."}[5m])) / sum by (service) (rate(http_requests_total[5m]))`
- **Rate Limited (req/s by route)** — `sum by (route) (rate(gateway_rate_limited_total[1m]))`

Auto-loaded by Grafana at startup (via provisioning).

### 3.3 Useful PromQL queries

```promql
# Request rate per service (last 5 min)
sum by (service) (rate(http_requests_total[5m]))

# p99 latency per service
histogram_quantile(0.99, sum by (le, service) (rate(http_request_duration_seconds_bucket[5m])))

# Rate limit hits per route (last 1 hour)
sum by (route) (increase(gateway_rate_limited_total[1h]))

# Top 5 slowest endpoints
topk(5, sum by (path) (rate(http_request_duration_seconds_sum[5m])) / sum by (path) (rate(http_request_duration_seconds_count[5m])))

# Error budget burn rate (5xx)
sum by (service) (rate(http_requests_total{status=~"5.."}[1h])) / sum by (service) (rate(http_requests_total[1h]))
```

---

## 4. Traces (OpenTelemetry)

### 4.1 Architecture

```
api-gateway    ──┐
token-service  ──┤── otel-collector ── Jaeger
iam-service    ──┤                     (UI :16686)
notification   ──┘

Each service:
- common-telemetry crate (init_tracing)
- OTel layer activated when OTEL_EXPORTER_OTLP_ENDPOINT set
- Exports spans via OTLP gRPC :4317
```

### 4.2 What gets traced

Every HTTP request produces:

```
http_request [root span]
├─ http.method, http.path, http.status_code, duration_ms
├─ otel.kind = "server"
└─ Child spans from:
   - auth_middleware (in api-gateway)
   - rate_limit_middleware (in api-gateway)
   - proxy_handler → outbound HTTP call (in api-gateway)
   - downstream service's http_request span (when OTel enabled)
```

### 4.3 Look up a trace

```bash
# 1. Get the X-Trace-Id from a response
curl -i http://localhost:3000/v1/iam/health | grep X-Trace-Id
# X-Trace-Id: 0af7651916cd43dd8448eb211c80319c

# 2. Open Jaeger UI directly to that trace
xdg-open "http://localhost:16686/trace/0af7651916cd43dd8448eb211c80319c"
```

In Jaeger UI:
- Service dropdown: `api-gateway` (or `token-service`, `iam-service`, `notification-service`)
- Operation: `http_request`
- Click "Find Traces"

### 4.4 Span attributes

Spans carry:

| Attribute | Example | Source |
|-----------|---------|--------|
| `http.method` | `POST` | tracing middleware |
| `http.path` | `/v1/iam/users` | tracing middleware |
| `http.status_code` | `200`, `429`, `500` | tracing middleware |
| `duration_ms` | `42` | tracing middleware |
| `otel.kind` | `server` | tracing middleware |
| `service.name` | `api-gateway` | OTel resource |
| `deployment.environment` | `dev` | OTel collector resource processor |

You can add custom attributes via `tracing::info_span!(..., custom_field = value)` in business code.

---

## 5. Cross-service trace chains

### 5.1 How propagation works

When `api-gateway` proxies to a downstream service:

1. Gateway's `proxy_handler` extracts the current span's context
2. Calls OTel's text map propagator (W3C TraceContext by default)
3. Injects `traceparent` header into the outbound request
4. Downstream service's tracing middleware extracts `traceparent`
5. Calls `span.set_parent(cx)` to chain spans

The result: **one trace in Jaeger** spanning all 4 services in the chain.

### 5.2 Example trace tree

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

### 5.3 OTel-enabled services (as of June 2026)

| Service | Status |
|---------|--------|
| api-gateway | ✅ Enabled |
| token-service | ✅ Enabled |
| iam-service | ✅ Enabled |
| notification-service | ✅ Enabled |
| prue_backend | — Not yet |
| audit-service | — Not yet |
| config-service | — Not yet |
| file-service | — Not yet |

To enable on a new service, see § 6 below.

### 5.4 trace_id in responses

Every HTTP response from an OTel-enabled service includes `X-Trace-Id` header:

```http
HTTP/1.1 200 OK
X-Trace-Id: 0af7651916cd43dd8448eb211c80319c
Content-Type: application/json
```

Even when the service has no OTel subscriber (i.e., tracing is OFF), the middleware falls back to a random 128-bit ID so client-side correlation still works.

---

## 6. Adding observability to a new service

### 6.1 Metrics (every service)

Every service already exposes `/metrics` via `prometheus` crate (P1.3 baseline). To add custom metrics:

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

// In handler:
MY_METRIC.with_label_values(&["x", "y"]).inc();
```

### 6.2 Traces (opt-in per service)

To enable OTel on a service that's not in the 4 enabled above:

**Step 1**: Update `Cargo.toml`:

```toml
common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }
opentelemetry = "0.24"
tracing-opentelemetry = "0.25"
rand.workspace = true
```

**Step 2**: Copy the middleware template:

```bash
cp services/api-gateway/src/middleware/tracing_mw.rs \
   services/<your-service>/src/tracing_mw.rs
# Adjust the path inside the file (api_gateway::... → <your_service>::...)
```

**Step 3**: In `src/lib.rs`:

```rust
pub mod tracing_mw;
```

**Step 4**: In `src/main.rs`:

```rust
common_telemetry::init_tracing("<your-service>");
```

**Step 5**: Wire the middleware in the Router:

```rust
.layer(axum::middleware::from_fn(
    <your_service>::tracing_mw::trace_request_middleware
))
```

**Step 6**: In `infrastructure/docker-compose.yml`:

```yaml
<your-service>:
  environment:
    - OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
```

**Step 7**: Verify with:

```bash
docker compose up -d
curl -i http://localhost:<port>/health | grep X-Trace-Id
# Open http://localhost:16686 and find the trace
```

### 6.3 Sampling (high-traffic scenarios)

Default: 100% sampling (every request traced). For high traffic, enable sampling:

```bash
# Sample 10% of traces
OTEL_TRACES_SAMPLER=TraceIdRatio
OTEL_TRACES_SAMPLER_ARG=0.1
```

Add to `common-telemetry` to make it configurable per environment.

---

## 7. Common issues

### 7.1 "No traces in Jaeger"

1. Check `OTEL_EXPORTER_OTLP_ENDPOINT` is set:
   ```bash
   docker exec api-gateway env | grep OTEL
   ```
2. Check the collector is reachable:
   ```bash
   docker exec api-gateway curl -v http://otel-collector:4317
   ```
3. Check collector logs:
   ```bash
   docker logs observability-otel-collector | tail -50
   ```
4. Check Jaeger received anything:
   ```bash
   curl http://localhost:16686/api/services
   ```

### 7.2 "Prometheus targets show all DOWN"

- Wait 15 seconds for first scrape
- Check the service has `metrics` tag in Consul:
  ```bash
  curl http://localhost:8500/v1/catalog/services | jq .
  ```
- Check service's `/metrics` endpoint directly:
  ```bash
  curl http://localhost:6002/metrics
  ```

### 7.3 "Trace has no spans"

- The OTel subscriber didn't initialize. Check `OTEL_EXPORTER_OTLP_ENDPOINT` is set when the service starts.
- Check service logs for `"OTel tracing enabled → ..."` message at startup.

### 7.4 "High memory / CPU usage"

Adjust batch parameters:

```bash
OTEL_BSP_SCHEDULE_DELAY=10000   # batch every 10s
OTEL_BSP_MAX_BATCH_SIZE=500      # max 500 spans per batch
```

Add to `common-telemetry::init_tracing` to make configurable per environment.

### 7.5 "Wanted to disable without redeploy"

Unset `OTEL_EXPORTER_OTLP_ENDPOINT` and restart the service. Without the env var, `common_telemetry` falls back to stdout-only JSON logs.

---

## 8. Reference

- `docs/guides/otel-tracing.md` — Detailed OTel setup
- `docs/guides/canary-rollout.md` — Canary rollout runbook
- `docs/USER_MANUAL.md` § 5 — Quick reference
- `infrastructure/config/prometheus/prometheus.yml` — Prometheus scrape config
- `infrastructure/config/otel/collector.yaml` — OTel collector pipeline
- `infrastructure/config/grafana/dashboards/gateway.json` — Dashboard JSON
- `libs/common-telemetry/src/lib.rs` — Source code reference