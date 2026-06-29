# coder-micro-services — User Manual

> Comprehensive guide to running, developing, deploying, and operating the
> coder-micro-services platform. Start here for any task.
>
> Last updated: 2026-06-25

---

## Table of Contents

1. [Quick Start](#1-quick-start)
2. [Architecture Overview](#2-architecture-overview)
3. [Local Development](#3-local-development)
4. [Testing](#4-testing)
5. [Observability](#5-observability)
6. [Deployment](#6-deployment)
7. [Rate Limiting](#7-rate-limiting)
8. [Canary Rollout](#8-canary-rollout)
9. [AI-Assisted Workflows](#9-ai-assisted-workflows)
10. [Troubleshooting](#10-troubleshooting)
11. [Reference](#11-reference)

---

## 1. Quick Start

### 1.1 Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Docker Desktop | 24+ | Runs MySQL / Redis / Consul / observability stack |
| Rust toolchain | 1.96+ (`rustup`) | Builds all Rust services |
| Node.js | 18+ | Runs CI gate scripts and e2e harness |
| pnpm | 8+ | Manages test workspaces |

### 1.2 First-time setup

```bash
# 1. Clone & install workspace deps
git clone <repo-url> coder-micro-services
cd coder-micro-services
pnpm install

# 2. Start infrastructure (MySQL / Redis / Consul)
docker compose -f infrastructure/docker-compose.yml up -d

# 3. (Optional) Start observability overlay
scripts/start-observability.sh

# 4. Verify infra is healthy
docker ps --filter "network=superpowers"
# All 8 services + consul + redis + mysql should be UP
```

### 1.3 Run a single service

```bash
# Pick a service (token / iam / notification / audit / config / file / prue_backend)
cd services/iam-service

# Run cargo tests (no local cc linker needed — uses docker)
node scripts/run-tests-docker.js iam-service

# Start the service
cargo run --bin iam-service
# → listens on 0.0.0.0:8080
# → Swagger UI at http://localhost:8080/swagger-ui (if SWAGGER_ENABLED=true)
```

### 1.4 Run end-to-end tests

```bash
# Smoke subset — 2 minutes
cd tests/e2e
pnpm install
pnpm test:smoke

# Full API tests — ~5 minutes
pnpm test:api

# Full UI tests — ~10 minutes (needs frontend container)
docker compose -f infrastructure/docker-compose.yml up -d frontend
pnpm test:ui
```

---

## 2. Architecture Overview

### 2.1 Service map

```
                            ┌──────────────────┐
   browser / SDK ─────────►│   api-gateway    │  :3000  (JWT + rate limit + tracing)
                            └────────┬─────────┘
                ┌──────────────────┼──────────────────────┐
                ▼                  ▼                      ▼
         token-service      iam-service         notification-service
         :6001              :6002                :6003   (WS push + tracing)
                │                  │                      │
                ▼                  ▼                      ▼
              MySQL              MySQL                  MySQL
                                + WS /ws
                ┌──────────────────┼──────────────────────┐
                ▼                  ▼                      ▼
         prue_backend       audit-service        config-service  file-service
         :6004              :6005                :6006           :6007
              SQLite/MySQL      MySQL               MySQL           MySQL
```

### 2.2 Layers

| Layer | Owns | Forbidden |
|-------|------|-----------|
| **api-gateway** | JWT verify, Redis blacklist, rate limiting, trace context injection | Business logic |
| **token-service** | Sign / refresh / revoke JWTs | User CRUD |
| **iam-service** | Identity + RBAC + dept tree | Issuing tokens directly |
| **notification-service** | Announcements + WebSocket push | Auth |
| **business services** | Each owns its bounded context | Calling each other directly (use gateway) |

### 2.3 Data flow (typical request)

```
1. Browser sends POST /v1/auth/login with credentials
2. Nginx (:80) proxies to api-gateway (:3000)
3. api-gateway:
   a. Tracing middleware: create root span, extract W3C traceparent
   b. Auth middleware: validate JWT (if required)
   c. Rate limit middleware: 5 rps for /v1/auth/login
   d. Proxy handler: route to iam-service (:6002), inject traceparent
4. iam-service: 
   a. Tracing middleware: chain span via traceparent
   b. Validate credentials, mint token via token-service (or via JWT lib)
   c. Return access_token + refresh_token
5. Response back: traceparent added on response so browser can correlate
6. Spans exported to OTel collector (if OTEL_EXPORTER_OTLP_ENDPOINT set)
7. Jaeger UI shows the full tree: api-gateway → iam-service
```

### 2.4 Service standard (8-item baseline)

Every Rust service must have (per ADR-002):

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ directory
- [x] `/health` endpoint
- [x] `/metrics` endpoint
- [x] migrations/ directory (exempt if no DB)
- [x] Swagger config (SWAGGER_ENABLED env switch)

CI gate: `node scripts/check-service-standard.js` exits 0 if all services comply.

### 2.5 Swagger UI coverage (8/8)

All 7 business services expose a full Swagger UI when `SWAGGER_ENABLED=true`. The `openapi.yaml` files are auto-generated from utoipa annotations on each handler.

| Service | Port | Swagger UI | OpenAPI YAML |
|---------|------|------------|--------------|
| token-service | 6001 | http://localhost:6001/swagger-ui | `services/token-service/openapi.yaml` |
| iam-service | 6002 | http://localhost:6002/swagger-ui | `services/iam-service/openapi.yaml` |
| notification-service | 6003 | http://localhost:6003/swagger-ui | `services/notification-service/openapi.yaml` |
| prue_backend | 6004 | http://localhost:6004/swagger-ui | `services/prue_backend/openapi.yaml` |
| audit-service | 6005 | http://localhost:6005/swagger-ui | `services/audit-service/openapi.yaml` |
| config-service | 6006 | http://localhost:6006/swagger-ui | `services/config-service/openapi.yaml` |
| file-service | 6007 | http://localhost:6007/swagger-ui | `services/file-service/openapi.yaml` |
| api-gateway | 3000 | (passthrough — see downstream UIs) | `services/api-gateway/openapi.yaml` |

Regenerate after annotation changes:
```bash
scripts/dump-openapi-specs.sh   # all 8 services
```

CI gate: `node scripts/check-spec.js` validates all 8 yaml files.

---

## 3. Local Development

### 3.1 Run services locally (not in docker)

```bash
# Terminal 1: infrastructure
docker compose -f infrastructure/docker-compose.yml up -d

# Terminal 2-9: services
for svc in token-service iam-service notification-service audit-service config-service file-service prue_backend; do
  (cd services/$svc && cargo run) &
done

# Or use pnpm parallel:
pnpm run dev
```

### 3.2 Hot-reload routes in api-gateway

```bash
# Edit infrastructure/config/api-gateway/routes/*.router.toml
# Then either:
kill -HUP $(pgrep api-gateway)          # SIGHUP
# Or POST to admin endpoint:
curl -X POST http://localhost:3000/__admin/reload
```

### 3.3 Debug with vscode

```bash
# Generate debug symbols
cargo build

# In vscode launch.json:
{
  "type": "lldb",
  "request": "launch",
  "name": "iam-service",
  "cargo": { "args": ["build", "--bin=iam-service"] },
  "args": [],
  "cwd": "${workspaceFolder}/services/iam-service"
}
```

### 3.4 Add a new dependency to a service

```bash
# Cargo.toml: add the dep
# Then verify it compiles:
node scripts/run-tests-docker.js iam-service
```

### 3.5 Edit contracts (error codes / permissions)

```bash
# 1. Edit contracts/error-codes.yaml or contracts/permissions.yaml
# 2. Regenerate Rust consumer code:
node scripts/contracts-gen.js

# 3. CI gate will fail if generated code drifts from YAML:
node scripts/contracts-gen.js --check
```

---

## 4. Testing

### 4.1 Test pyramid

```
┌────────────────────────────────────┐
│  E2E (Playwright UI + Vitest API)  │  ← real services in docker
│  - Smoke: 2 min (8 API + 2 UI)    │
│  - Full API: ~5 min                │
│  - Full UI: ~10 min                │
├────────────────────────────────────┤
│  Rust Integration Tests            │  ← cargo test --test <name>
│  - sqlx / mockall / testcontainers│
├────────────────────────────────────┤
│  Rust Unit Tests                   │  ← cargo test --lib
│  - Service logic, no I/O           │
└────────────────────────────────────┘
```

### 4.2 Run all tests for a service

```bash
node scripts/run-tests-docker.js iam-service
# Equivalent to:
docker run --rm --network host \
  -v $(pwd):/app \
  -v coder-cargo-target:/app/target \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -w /app \
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl \
  cargo test --target x86_64-unknown-linux-musl -p iam-service
```

### 4.3 Run a specific test

```bash
# Rust integration test by name
cargo test --test integration_test -- --nocapture test_login

# E2E smoke subset
cd tests/e2e && E2E_SMOKE=1 pnpm test:api
```

### 4.4 E2E smoke vs full

| Tier | Time | Cases | When |
|------|------|-------|------|
| **smoke** | 2 min | 8 API + 2 UI | Pre-commit, dev iteration |
| **api** | 5 min | All API (50+) | Pre-deploy, CI |
| **ui** | 10 min | All UI (20+) | Pre-deploy, CI |

Cases with `smoke: true` flag run in smoke subset. Mark critical happy-path cases as smoke.

### 4.5 Add a new E2E test (data-driven)

```yaml
# tests/e2e/api/cases/iam-users.yaml
- caseId: IAM-USER-042
  desc: New test case description
  method: POST
  path: /v1/iam/users
  body:
    username: new${rand}
    password: P@ssw0rd!
  expectStatus: 200
  expect:
    - $.id exists
  smoke: true   # include in smoke subset
```

See `docs/guides/testing.md` for the full data-driven test format.

---

## 5. Observability

### 5.1 Stack

```
┌─────────────────────────────────────────────────┐
│  Services (api-gateway + 4 OTel-enabled)        │
│   ↓ OTLP gRPC :4317                             │
├─────────────────────────────────────────────────┤
│  otel-collector (batch / resource enrich)       │
│   ↓                              ↓              │
│  Jaeger (traces UI) :16686    Prometheus :8889  │
│   :14250 (gRPC receiver)       :9090 (scrape)    │
└─────────────────────────────────────────────────┘
       ↑                                 ↑
       │                                 │
  http://localhost:16686/t/...   http://localhost:9090
```

### 5.2 Bring up observability

```bash
# 1. Start infra + services
docker compose -f infrastructure/docker-compose.yml up -d

# 2. Start observability overlay (Prometheus + Grafana + OTel collector + Jaeger)
scripts/start-observability.sh

# 3. Access UIs:
#    Prometheus :9090      — metrics scraping
#    Grafana    :3001      — dashboards (admin/admin)
#    Jaeger     :16686     — traces UI
#    OTel collector OTLP gRPC :4317 (internal)
```

### 5.3 Enable OTel on a service

Already enabled (4 services): api-gateway, token-service, iam-service, notification-service. They have `OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317` in `infrastructure/docker-compose.yml`.

To enable on another service:
1. Add `common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }` to Cargo.toml
2. Add `common-telemetry-axum = { path = "../../libs/common-telemetry-axum", features = ["otlp"] }` for the HTTP middleware
3. Add `common_telemetry::init_tracing("service-name")` in main.rs
4. Add `.route_layer(axum_mw::from_fn(common_telemetry_axum::trace_request_middleware))`
5. Set `OTEL_EXPORTER_OTLP_ENDPOINT` env var in docker-compose

See `docs/guides/otel-tracing.md` for full details.

### 5.4 Look up a trace by ID

```bash
# Get the X-Trace-Id from a response
curl -i http://localhost:3000/v1/iam/health | grep X-Trace-Id
# X-Trace-Id: 0af7651916cd43dd8448eb211c80319c

# Open Jaeger UI:
#   http://localhost:16686/trace/0af7651916cd43dd8448eb211c80319c
```

### 5.5 Query metrics

```promql
# Request rate per service
sum by (service) (rate(http_requests_total[1m]))

# Error rate (5xx)
sum by (service) (rate(http_requests_total{status=~"5.."}[5m])) / sum by (service) (rate(http_requests_total[5m]))

# Rate-limited requests by route
sum by (route) (rate(gateway_rate_limited_total[1m]))
```

See Grafana → Gateway Overview dashboard for pre-built panels.

---

## 6. Deployment

### 6.1 Build images

```bash
# All services
pnpm run docker:build

# Single service
pnpm run docker:build:iam-service

# Image is tagged and pushed to internal registry:
#   zhcoder-docker-registry.com:8000/micro-server/<service>:<commit-sha>
```

### 6.2 Promote to production

GitLab CI pipeline (`test` → `build` → `deploy`):
1. **test**: `service_standard_check`, `spec_check`, `contracts_check`, `spec_generate`, `e2e_api_test`, `e2e_ui_test`, `rust_test`
2. **build**: Push Docker images tagged with `CI_COMMIT_SHORT_SHA` and `latest`
3. **deploy**: SSH to deploy server, pull images, restart services

Manual deploy:
```bash
ssh kudi@192.168.2.128
cd /opt/coder-micro-services
docker-compose -f infrastructure/docker-compose.yml down
docker-compose -f infrastructure/docker-compose.yml pull
docker-compose -f infrastructure/docker-compose.yml up -d
```

### 6.3 CI gates (must all pass)

| Gate | Script |
|------|--------|
| Service Standard | `node scripts/check-service-standard.js` |
| Spec Check | `node scripts/check-spec.js` |
| Contracts Check | `node scripts/contracts-gen.js --check` |
| OpenAPI Drift | `scripts/dump-openapi-specs.sh` + `git diff --exit-code` |
| Rust Tests | `node scripts/run-tests-docker.js <svc>` |
| E2E API | `pnpm -C tests/e2e test:api` |
| E2E UI | `pnpm -C tests/e2e test:ui` |

Run all gates locally before push:
```bash
make ci  # if Makefile exists; otherwise:
node scripts/check-service-standard.js && \
  node scripts/check-spec.js && \
  node scripts/contracts-gen.js --check && \
  scripts/dump-openapi-specs.sh && \
  git diff --exit-code -- 'services/*/openapi.yaml'
```

---

## 7. Rate Limiting

### 7.1 How it works — dual-layer architecture

Rate limiting is **two independent layers**:

**Layer 1: Gateway (P3.2)** — coarse route-level limits
- Algorithm: in-memory `governor` token bucket
- Identity: user_id → X-Real-IP → peer IP
- Scope: anti-DDoS, anti-brute-force, fair-use
- Storage: per-instance in-memory
- Config: `infrastructure/config/api-gateway/rate-limits.toml`

**Layer 2: Service-side (P3.2b)** — fine business-logic limits
- Algorithm: Redis sliding-window ZSET
- Identity: X-User-Id → Authorization header → peer IP
- Scope: business rules (e.g. "5 announcements per admin per minute")
- Storage: shared across replicas via Redis
- Config: hard-coded in service `default_limits()` for now

```
Request
  ↓
[Nginx]
  ↓
[api-gateway]
  ├─ JWT validation (X-User-Id injection)
  ├─ Gateway rate limit (route-level, in-memory)
  ├─ Blacklist check
  └─ Proxy handler (injects traceparent)
      ↓
  [Downstream service, e.g. notification-service]
      └─ Service-side rate limit (business-logic, Redis)
          ↓
      [Handler]
```

A request passes both layers, or fails one and gets 429.

### 7.2 Gateway configuration

`infrastructure/config/api-gateway/rate-limits.toml`:

```toml
[defaults]
requests_per_second = 100
burst = 200

[routes."/v1/iam/auth/login"]
requests_per_second = 5
burst = 10
```

Edit and restart the gateway to apply changes.

### 7.3 Service-side configuration (notification-service example)

In `services/notification-service/src/rest.rs`:

```rust
pub fn default_limits() -> Vec<Limit> {
    vec![
        Limit::new("POST /notification/announcements", 5, 60),      // anti-spam
        Limit::new("POST /notification/announcements/{id}/publish", 10, 60),
        Limit::new("POST /notification/announcements/{id}/revoke", 10, 60),
    ]
}
```

Currently `notification-service` is the first service with service-side limits. Other services can opt in by:

1. Add `common-ratelimit = { path = "../../libs/common-ratelimit" }` to Cargo.toml
2. Create `RateLimiter::new(redis.conn.clone(), default_limits())`
3. Add `.route_layer(axum::middleware::from_fn_with_state(rate_limiter, rate_limit_middleware))`
4. Call `common_ratelimit::init_metrics("service-name")` in main

### 7.4 Response when limited

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 60
Content-Type: application/json

{
  "code": "RATE_LIMITED",
  "message": "too many requests",
  "retry_after_ms": 60000
}
```

### 7.5 Failure mode

**Service-side rate limiter fails open**: if Redis is unreachable, the request is allowed (with a `warn` log). Reason: Redis going down shouldn't break the whole product. Gateway rate limiter (in-memory) still works.

### 7.6 Observability

| Metric | Layer | Labels |
|--------|-------|--------|
| `gateway_rate_limited_total` | gateway | route |
| `service_rate_limited_total` | service | service, route |

Both visible in Grafana → Gateway Overview → "Rate Limited (req/s by route)" panel (the service metric will need its own panel once more services opt in).

---

## 8. Canary Rollout

### 8.1 Mechanism

- Consul service discovery tags an instance as `canary`
- Nginx `split_clients $request_id` routes 10% of traffic to canary instances
- `?canary=1` query or `X-Canary: true` header forces canary for testing
- Health check: Consul TTL removes unhealthy canary from upstream

### 8.2 Deploy canary instance

```bash
# Tag an existing instance as canary (quick test)
docker exec consul consul tags -add -service audit-service -tag canary -id <instance-id>

# Or deploy a separate canary instance via compose
docker compose -f infrastructure/docker-compose-canary.yml up -d audit-service-canary
```

### 8.3 Verify in Jaeger

```bash
# Hit the service many times; ~10% should hit canary
for i in {1..100}; do curl -s http://localhost:3000/v1/audit/health; done

# Or use X-Canary header to force
curl -H "X-Canary: true" http://localhost:3000/v1/audit/health
# → Trace in Jaeger shows instance=audit-service-canary
```

### 8.4 Rollback (1 second)

```bash
docker compose -f infrastructure/docker-compose-canary.yml down
# OR
docker exec consul consul tags -delete -service audit-service -tag canary -id <instance-id>
# → Within ~10s, all traffic returns to 100% stable
```

Full runbook: `docs/guides/canary-rollout.md`

---

## 9. AI-Assisted Workflows

### 9.1 Skill catalog

Project skills (`.agents/skills/`):

| Skill | Use When |
|-------|---------|
| `coder-add-route` | Adding a new endpoint to an existing service |
| `coder-migrate-db` | Adding a new DB migration (MySQL + SQLite) |
| `coder-new-service` | Scaffolding a brand-new Rust microservice |

Platform skills (`.trae/skills/`):

| Skill | Use When |
|-------|---------|
| `coder-base-images-sync` | Syncing base Docker images |
| `coder-dev-bootstrap` | Setting up local dev environment |
| `coder-e2e-test` | Running e2e tests against infrastructure |
| `coder-lib-upgrade` | Upgrading shared Rust libraries |
| `coder-service-audit` | Auditing services against AGENTS.md standards |
| `plan-eng-review` | Reviewing plans in eng-manager mode |

### 9.2 L4 triggers

The AI automatically loads skills based on what files you touch. See `.trae/rules/project_rules.md` for the full trigger map.

| Path | Auto-load |
|------|-----------|
| `services/*/src/routes/*.rs` | `coder-add-route` (for new routes) |
| `services/*/migrations/**` | `coder-migrate-db` (for new migrations) |
| `services/*/openapi.yaml` | Warning: don't edit directly; run `scripts/dump-openapi-specs.sh` |
| `contracts/**` | Run `node scripts/contracts-gen.js` |

### 9.3 Spec discipline

Every code change must reference a spec in `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`. Spec docs follow the convention `YYYY-MM-DD-<topic>-design.md`.

### 9.4 Pre-commit hook

```bash
# Recommended: pre-commit framework
pre-commit install

# OR: zero-dep fallback
cp scripts/git-hooks/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
```

The hook runs service-standard / spec / contracts / openapi-drift gates before each commit.

---

## 10. Troubleshooting

### 10.1 Common errors

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| `cargo: linker 'cc' not found` | Local toolchain missing | Use `node scripts/run-tests-docker.js <svc>` |
| `api-gateway` blacklist test fails | Pre-existing Redis dep | Run via docker (uses test Redis) |
| utoipa `to_yaml()` method not found | Missing `yaml` feature | Add `features = ["axum_extras", "chrono", "yaml"]` to utoipa |
| 429 Too Many Requests | Rate limit hit | Adjust `infrastructure/config/api-gateway/rate-limits.toml` |
| OpenAPI drift CI failure | Hand-edited yaml | Run `scripts/dump-openapi-specs.sh` and commit |
| Service won't register in Consul | Container not on `superpowers` network | Add `networks: [superpowers]` in compose |
| Trace not in Jaeger | `OTEL_EXPORTER_OTLP_ENDPOINT` unset | Set in compose or env var |
| Swagger 404 | `SWAGGER_ENABLED=false` | Set env to `true` |
| `iam_outbox_events` table growing unbounded | Archive cron runs every 5 min; live rows >24h are moved to `iam_outbox_events_archive`, which is purged after 30 days. Check `iam-outbox-publisher` logs for archive cycle counts. |

### 10.2 Reset everything

```bash
# Nuclear: tear down all infra + observability
docker compose -f infrastructure/docker-compose.yml down -v
docker compose -f infrastructure/docker-compose-observability.yml down -v
docker network rm superpowers 2>/dev/null

# Then start fresh:
docker compose -f infrastructure/docker-compose.yml up -d
scripts/start-observability.sh
```

### 10.3 Debug specific request

```bash
# 1. Send request, capture trace ID
TRACE_ID=$(curl -s -i http://localhost:3000/v1/iam/health | awk '/X-Trace-Id:/ {print $2}' | tr -d '\r')

# 2. Open Jaeger with this trace
xdg-open "http://localhost:16686/trace/$TRACE_ID"  # Linux
open "http://localhost:16686/trace/$TRACE_ID"      # macOS

# 3. Inspect logs for trace_id
docker logs api-gateway 2>&1 | grep "$TRACE_ID"
```

---

## 11. Reference

### 11.1 Key documents

| Document | Purpose |
|----------|---------|
| `docs/ai-context.md` | AI entrypoint — service map, pitfalls, commands |
| `docs/decisions/ADR-*.md` | Architecture Decision Records |
| `docs/guides/canary-rollout.md` | Canary deployment runbook |
| `docs/guides/otel-tracing.md` | OpenTelemetry setup + usage |
| `docs/guides/testing.md` | E2E test data-driven format |
| `docs/superpowers/plans/2026-06-25-overall-roadmap.md` | 12-week roadmap |
| `docs/superpowers/specs/` | Design specs (one per change) |
| `docs/superpowers/retros/` | Monthly retrospectives |
| `services/<svc>/AGENTS.md` | Per-service conventions |

### 11.2 File layout

```
coder-micro-services/
├── README.md                   ← this file
├── agents.md                   ← global agent rules
├── Cargo.toml                  ← workspace root
├── infrastructure/             ← docker compose + configs
│   ├── docker-compose.yml      ← main infra
│   ├── docker-compose-observability.yml  ← observability overlay
│   └── config/
│       ├── prometheus/
│       ├── grafana/
│       ├── nginx/
│       ├── otel/
│       └── api-gateway/
│           ├── routes/*.router.toml  ← API routes
│           └── rate-limits.toml       ← rate limit config
├── services/                   ← 8 Rust services
│   └── <svc>/
│       ├── Cargo.toml
│       ├── AGENTS.md
│       ├── Dockerfile
│       ├── openapi.yaml         ← auto-generated
│       ├── migrations/
│       └── src/
├── libs/                       ← 13 shared Rust crates
│   └── common-telemetry/        ← OTel scaffolding
├── frontend/                   ← Vue 3 admin
├── tests/e2e/                  ← Vitest + Playwright
├── contracts/                  ← single source of truth (YAML)
├── docs/                       ← this manual + guides + retros
├── scripts/                    ← JS automation
│   ├── check-service-standard.js
│   ├── check-spec.js
│   ├── contracts-gen.js
│   ├── run-tests-docker.js
│   ├── dump-openapi-specs.sh
│   └── start-observability.sh
├── .agents/skills/             ← AI skills (project)
└── .trae/                      ← AI rules + platform skills
```

### 11.3 Common shortcuts

```bash
# Aliases (add to your shell rc)
alias cms-ci='node scripts/check-service-standard.js && node scripts/check-spec.js && node scripts/contracts-gen.js --check'
alias cms-openapi='scripts/dump-openapi-specs.sh'
alias cms-test='node scripts/run-tests-docker.js'
alias cms-obs='scripts/start-observability.sh'
```

### 11.4 Glossary

| Term | Meaning |
|------|---------|
| **Consul** | Service discovery + health check |
| **JWT** | JSON Web Token (RS256 signed) |
| **OTel / OTLP** | OpenTelemetry / OpenTelemetry Protocol |
| **Pure-admin** | Vue 3 admin template this project forks |
| **prue_backend** | The pure-admin menu config backend (Rust port) |
| **smoke subset** | Critical-path tests for fast feedback |
| **trace_id** | 128-bit ID propagated via W3C `traceparent` |
| **WS / WebSocket** | Persistent bidirectional channel for push notifications |

---

_This manual is maintained alongside the codebase. Last reviewed: 2026-06-25 (after Phase 3 + OTel expansion + skills completion)._