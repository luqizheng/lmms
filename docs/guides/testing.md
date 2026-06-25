# Testing Guide

> Last updated: 2026-06-25

This document covers:
1. The 3-tier test pyramid (unit → integration → e2e)
2. Running tests locally and in CI
3. Data-driven e2e test format
4. Smoke subset for fast feedback
5. Pre-flight health checks

---

## 1. Test pyramid

```
        ┌──────────────────────────────────────┐
   e2e  │  Playwright UI + Vitest API         │  ~10 min full / ~2 min smoke
        │  Real services in docker             │
        ├──────────────────────────────────────┤
  intg  │  Rust cargo test --test <name>       │  per-service ~30s
        │  sqlx / mockall / testcontainers     │
        ├──────────────────────────────────────┤
  unit  │  Rust cargo test --lib               │  per-service ~5s
        │  Service logic, no I/O                │
        └──────────────────────────────────────┘
```

| Tier | Time (per service / full) | Use when |
|------|---------------------------|----------|
| Unit | ~5s | Pure logic, no DB / network |
| Integration | ~30s | DB / network / testcontainers |
| E2E (smoke) | ~2 min | Critical happy path |
| E2E (API full) | ~5 min | All API contracts |
| E2E (UI full) | ~10 min | All user flows |

---

## 2. Running tests

### 2.1 Unit + integration (per service)

```bash
# All tests for a service (uses docker, no local cc linker needed)
node scripts/run-tests-docker.js iam-service

# Multiple services
node scripts/run-tests-docker.js iam-service token-service notification-service

# All services
node scripts/run-tests-docker.js
```

Equivalent low-level docker command:

```bash
docker run --rm --network host \
  -v $(pwd):/app \
  -v coder-cargo-registry:/usr/local/cargo/registry \
  -v coder-cargo-git:/usr/local/cargo/git \
  -v coder-cargo-target:/app/target \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -w /app \
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl \
  cargo test --target x86_64-unknown-linux-musl -p iam-service -- --nocapture
```

### 2.2 Run a specific test by name

```bash
# Integration test
cargo test --test integration_test -- --nocapture test_login

# Unit test
cargo test --lib -- --nocapture create_user
```

### 2.3 E2E tests

```bash
cd tests/e2e
pnpm install  # first time only

# Smoke subset (fast, ~2 min)
pnpm test:smoke           # API smoke
pnpm test:smoke:ui        # UI smoke (with @smoke tag)

# Full API
pnpm test:api

# Full UI (requires frontend container)
docker compose -f infrastructure/docker-compose.yml up -d frontend
pnpm test:ui
```

### 2.4 Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `E2E_BASE_URL` | `http://localhost:3000` | api-gateway URL |
| `E2E_FRONTEND_URL` | `http://localhost:8080` | Frontend URL (UI tests) |
| `E2E_ADMIN_USER` | `e2eadmin` | Auto-registered admin user |
| `E2E_ADMIN_PASS` | `P@ssw0rd!` | Admin password |
| `E2E_DEBUG` | `0` | Set to `1` for request/response logs |
| `E2E_SMOKE` | unset | Set to `1` to filter to smoke subset |
| `E2E_RETRY` | `1` | Override retry count for vitest |

---

## 3. Data-driven E2E test format

All e2e cases live in YAML, one file per service or business flow.

### 3.1 API case format

`tests/e2e/api/cases/<service>.yaml`:

```yaml
group: <service-group-name>
cases:
  - caseId: <PREFIX>-<NUMBER>
    desc: <human description>
    smoke: true                       # include in smoke subset (optional)
    method: POST | GET | PUT | DELETE | PATCH
    path: /v1/iam/users
    auth: false                       # default true; false for unauthenticated
    headers:                          # optional extra headers
      X-Trace-Id: <32-hex>
    body:                             # request body (any JSON)
      username: new${rand}            # ${rand} generates unique value
      password: P@ssw0rd!
    expectStatus: 200                 # or [200, 201] for tolerant
    expect:                           # optional body assertions
      - $.id exists                   # path + operator + value
      - $.username==new${rand}
    timeoutMs: 30000                  # override default 20s
    skip: false                       # mark as skipped
```

### 3.2 UI case format

`tests/e2e/ui/cases/<flow>.yaml`:

```yaml
group: <ui-group-name>
cases:
  - caseId: UI-<PREFIX>-<NUMBER>
    desc: <description>
    smoke: true
    tags: ['@smoke']                  # for playwright grep
    flow: <flow-name>                 # must exist in tests/e2e/ui/flows/
    params:                           # passed to flow function
      title: e2e-test-${rand}
    expect:                           # UI assertions
      - toast=success                 # toast contains "success"
      - dialog=created                # dialog contains "created"
```

### 3.3 Assertion expressions

The runner supports 5 assertion operators in `expect`:

| Expression | Meaning |
|------------|---------|
| `$.field exists` | Field is present and not null |
| `$.field not exists` | Field is absent or null |
| `$.field is array` | Field is a JSON array |
| `$.field contains <substr>` | Field (stringified) contains substring |
| `$.field==<value>` | Field equals value (string compare) |

### 3.4 Placeholders

| Placeholder | Replaced with |
|-------------|---------------|
| `${rand}` | Random `Date.now() + 4-char hex` |
| `${case-id.field}` | Reference to prior case's response field |

Example: `${IAM-USER-001.id}` references the `id` field returned by case `IAM-USER-001`.

---

## 4. Smoke subset

### 4.1 What is smoke?

Smoke is a curated subset of critical-path cases marked with `smoke: true` in YAML. Purpose: fast feedback loop during development.

| Tier | Time | Cases | When to run |
|------|------|-------|-------------|
| **smoke** | ~2 min | 8 API + 2 UI | Pre-commit, dev iteration |
| **api** | ~5 min | 50+ | Pre-deploy, CI |
| **ui** | ~10 min | 20+ | Pre-deploy, CI |

### 4.2 Running smoke

```bash
cd tests/e2e

# Smoke via env var
E2E_SMOKE=1 pnpm test:api

# Or via dedicated script
pnpm test:smoke
pnpm test:smoke:ui    # uses --grep @smoke in playwright
```

### 4.3 Adding a case to smoke

In the case YAML, set `smoke: true`:

```yaml
cases:
  - caseId: NOTIF-001
    desc: Create announcement
    smoke: true       # ← include in smoke subset
    method: POST
    ...
```

For UI tests, also add the tag:

```yaml
- caseId: UI-NOTIF-001
  smoke: true
  tags: ['@smoke']   # ← for playwright --grep
  ...
```

### 4.4 What makes a good smoke case

- Critical happy path of one endpoint
- No dependencies on prior cases (or minimal)
- Fast (no large uploads / long waits)
- Validates a real production scenario

Mark these as smoke:
- `AUTH-001` (login works)
- `GW-001` (gateway routing works)
- `HEALTH` (basic health check)
- `IAM-USER-001` (user creation)
- One critical path per service

Don't mark these as smoke:
- Tests that depend on many prior cases
- Edge cases / error paths
- Bulk operations

---

## 5. Pre-flight health checks

Before any e2e test runs, the harness verifies infrastructure is up. This prevents cascading failures that look like test failures but are actually infra problems.

### 5.1 What gets checked

`tests/e2e/shared/health.ts` probes:

| Probe | URL | Expected |
|-------|-----|----------|
| `api-gateway /health` | `${E2E_BASE_URL}/health` | 200 |
| `iam via gateway` | `${E2E_BASE_URL}/iam/health` | 200 |

Each probe has a 3-second timeout.

### 5.2 When it fails

```
preflight api-gateway /health FAILED: ...
  → 请先启动 infrastructure: docker compose -f infrastructure/docker-compose.yml up -d
```

Fix: ensure infrastructure is running:

```bash
docker compose -f infrastructure/docker-compose.yml up -d
```

Then re-run tests.

### 5.3 Why preflight matters

Without preflight, a broken infra would cause 50+ test failures with misleading "test broken" messages. With preflight, you get one clear error pointing to the root cause.

---

## 6. Retry policy

### 6.1 Vitest

Default retry: 1 (test re-runs once if it fails). Override with `E2E_RETRY=N`:

```bash
E2E_RETRY=0 pnpm test:api      # no retry (CI stricter)
E2E_RETRY=3 pnpm test:smoke    # more retry (flaky network)
```

### 6.2 Playwright

Default: `retries: 0` locally, `retries: 2` in CI (set via `process.env.CI`).

### 6.3 When retries help vs hide bugs

Retries help with:
- Transient network failures (MySQL pool reset)
- Timing-sensitive tests (WS message arrival)

Retries hide:
- Race conditions in the production code (e.g., etag conflict — should not retry, should fail clearly)
- Missing test data setup

If a test consistently needs retries to pass, that's a smell — investigate the underlying flakiness rather than masking it.

---

## 7. Reports

### 7.1 Excel report

After each e2e run, an Excel report is generated at:

```
tests/e2e/shared/report/report-<timestamp>.xlsx
```

Sheets:
- **明细 (Details)**: case-by-case result, status, duration, error message
- **汇总 (Summary)**: total / pass / fail / skip counts

### 7.2 Error details

For each failed case, a JSON file is written to:

```
tests/e2e/shared/report/errors/<caseId>.json
```

Contents:
```json
{
  "caseId": "IAM-USER-002",
  "request": { "method": "GET", "path": "...", "headers": {}, "body": null },
  "response": { "status": 500, "body": {} },
  "expect": { "status": 200, "body": "$.username==..." },
  "timestamp": "..."
}
```

UI failures also include a screenshot at:
```
tests/e2e/shared/report/errors/<caseId>.png
```

### 7.3 Reading the report

```bash
# macOS
open tests/e2e/shared/report/report-*.xlsx

# Linux
xdg-open tests/e2e/shared/report/report-*.xlsx

# Or programmatically:
python3 -c "import openpyxl; wb = openpyxl.load_workbook('tests/e2e/shared/report/report-latest.xlsx'); print(wb.sheetnames)"
```

---

## 8. CI integration

### 8.1 GitLab CI jobs

`.gitlab-ci.yml` defines:

| Job | Stage | Purpose |
|-----|-------|---------|
| `rust_test` | test | `cargo test --workspace --all-targets` |
| `e2e_api_test` | test | `pnpm -C tests/e2e run test:api` |
| `e2e_ui_test` | test | `pnpm -C tests/e2e run test:ui` (needs frontend) |
| `service_standard_check` | test | Gate |
| `spec_check` | test | Gate |
| `contracts_check` | test | Gate |

### 8.2 Pre-commit hooks

Configured via `.pre-commit-config.yaml` (pre-commit framework) or `scripts/git-hooks/pre-commit` (zero-dep fallback).

The hook runs service-standard / spec / contracts / openapi-drift gates before each commit. E2E is NOT run in pre-commit (too slow); use `pnpm test:smoke` for fast local feedback.

---

## 9. Common patterns

### 9.1 Reuse context across cases

```yaml
# First case creates a user
- caseId: USER-CREATE
  method: POST
  path: /v1/iam/users
  body:
    username: alice-${rand}
  expectStatus: 200
  expect:
    - $.id exists

# Second case uses the created user's ID
- caseId: USER-GET
  method: GET
  path: /v1/iam/users/${USER-CREATE.id}  # references prior case
  expectStatus: 200
  expect:
    - $.username matches alice-*
```

### 9.2 Tolerate multiple acceptable statuses

```yaml
expectStatus: [200, 201]   # either is acceptable
```

Useful when:
- Service returns 200 sometimes, 201 other times
- New service returns 200, old service returns 201 (during migration)

### 9.3 Test multi-step business flows

Use UI cases for full business flows, API cases for individual endpoint contracts.

```yaml
# UI: create + publish + verify
- caseId: UI-NOTIF-FULL
  flow: createAndPublish
  params:
    title: e2e-${rand}
  expect:
    - toast=已发布
```

The flow implementation lives in `tests/e2e/ui/flows/index.ts`.

---

## 10. Debugging

### 10.1 Enable verbose logging

```bash
E2E_DEBUG=1 pnpm test:api
```

Logs every request/response to stderr.

### 10.2 Run a single case

```bash
# Vitest
cd tests/e2e
pnpm vitest run --config api/vitest.config.ts -t "AUTH-001"

# Playwright
pnpm playwright test --config ui/playwright.config.ts --grep "UI-IAM-CHPW-001"
```

### 10.3 Inspect test state

```bash
# Tail runner output
node scripts/run-tests-docker.js iam-service 2>&1 | tail -50

# Check infrastructure health
docker ps
docker logs <container>
curl http://localhost:3000/health
```

### 10.4 Common issues

| Issue | Fix |
|-------|-----|
| Test "passes locally, fails in CI" | Check env vars; CI may have different MySQL/Redis versions |
| Cascading failures (50 tests fail) | Likely infra; check preflight output |
| Test "fails on first run, passes on retry" | Race condition; investigate rather than increase retry |
| Screenshot shows blank page | Frontend container not started; `docker compose up -d frontend` |

---

_This guide is maintained alongside the codebase. See `docs/superpowers/specs/2026-06-25-e2e-stability-design.md` for the design rationale._