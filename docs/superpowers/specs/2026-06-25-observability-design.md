# P3.1 Observability Stack — Prometheus + OTel Traces

## Decision

**Adopt recommended path: Prometheus for metrics (immediate) + OpenTelemetry traces (scaffolded, opt-in per service).**

## Context

All 8 services already expose `/metrics` (Prometheus text format) per P1.3 baseline. No service discovery / scrape config, no tracing, no centralized observability. Goal: get metrics scraped + traces pipeline ready without rewriting existing service code.

## Components

### 1. Prometheus with Consul Service Discovery

- New service in `infrastructure/docker-compose.yml`: `prometheus:prometheus:v2.51.0`
- Scrape config: `infrastructure/config/prometheus/prometheus.yml`
- Uses Consul SD to auto-discover all 8 service `/metrics` endpoints
- Relabel rule: keep only services tagged `metrics=true`

### 2. Grafana with two starter dashboards

- `grafana/grafana:10.4.0` service
- Two provisioned dashboards (committed as JSON):
  - `gateway.json` — request rate, latency p50/p95/p99, error rate by route
  - `services.json` — per-service request rate, DB connection pool

### 3. OTel Traces — scaffolded, not mandatory

- Add `opentelemetry`, `opentelemetry-otlp`, `tracing-opentelemetry` to workspace deps
- New crate `libs/common-telemetry` provides `init_tracing(otlp_endpoint: Option<String>)` 
  - If endpoint is None: only `tracing-subscriber` (current behavior)
  - If endpoint is Some: layer OTel exporter over tracing-subscriber
- Only `api-gateway` opts in initially (via `OTEL_EXPORTER_OTLP_ENDPOINT` env)
- Other services can opt in by setting the env var; no default behavior change

### 4. Local dev experience

- `node scripts/base-images-sync.js` already handles base images; add `scripts/start-observability.sh`
- One command brings up: `docker compose -f infrastructure/docker-compose-observability.yml up -d`
- Standalone file to avoid polluting main compose

## Files Touched

| File | Change |
|------|--------|
| `infrastructure/config/prometheus/prometheus.yml` | NEW |
| `infrastructure/docker-compose-observability.yml` | NEW (overlay) |
| `infrastructure/config/grafana/provisioning/datasources/prometheus.yml` | NEW |
| `infrastructure/config/grafana/provisioning/dashboards/dashboards.yml` | NEW |
| `infrastructure/config/grafana/dashboards/{gateway,services}.json` | NEW |
| `scripts/start-observability.sh` | NEW |
| `libs/common-telemetry/Cargo.toml` | NEW crate |
| `libs/common-telemetry/src/lib.rs` | NEW init_tracing function |
| `Cargo.toml` (workspace) | Add libs/common-telemetry to members |
| `services/api-gateway/Cargo.toml` | Optional dep on common-telemetry |
| `services/api-gateway/src/main.rs` | Call init_tracing with optional OTLP |
| `services/api-gateway/src/middleware/mod.rs` | Add trace_id extraction |
| `docs-site/ai-context.md` | Update § 9 CI gates + § 2 service map with observability services |
| `docs/superpowers/specs/2026-06-25-observability-design.md` | NEW (this doc) |

## Verification

1. Start infrastructure + observability overlay
2. Curl each service `/metrics` → verify Prometheus target page shows them
3. Grafana dashboard shows request rate spikes when curl-ing api-gateway
4. With OTLP endpoint set, `api-gateway` spans show up in Jaeger / Tempo
5. Without OTLP endpoint set, `api-gateway` works identically to before (zero regression)