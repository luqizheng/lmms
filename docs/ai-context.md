# AI Context — coder-micro-services

> **AI agents MUST read this file first** when starting work in this repo.
> It is the single source of truth for project shape, conventions, and pitfalls.
> Updated by P3.5 (2026-06-25).

---

## 1. Project Snapshot

`coder-micro-services` is a low-memory microservice platform built around a Vue 3 admin frontend and 8 Rust backend services (api-gateway + 7 business services). It uses Consul for service discovery, Redis for JWT blacklist / caching, MySQL for persistent data, and SQLite for the pure-admin menu-config service. Multi-language SDKs (Java, .NET, Node) are published to internal Maven and NuGet feeds.

- **Repo layout**: monorepo with `services/` (Rust), `frontend/` (Vue 3), `libs/` (Rust shared libs), `tests/e2e/` (Playwright + Vitest), `contracts/` (single source of truth for error codes / permissions / events).
- **Current phase**: end of Phase 3 — service standard baseline, OpenAPI auto-dump, L4 triggers, e2e smoke subset, knowledge consolidation all live.
- **Branch policy**: all work goes on `main`, one task per commit, no feature branches.

---

## 2. Service Map

| Service | Port | DB | Tech | Role |
|---------|------|----|----|------|
| `api-gateway` | 3000 | — | Axum | JWT verify, Redis blacklist, route to downstream |
| `token-service` | 6001 | MySQL | Axum | Issue / refresh / revoke JWTs |
| `iam-service` | 6002 | MySQL | Axum | Users, roles, permissions, departments, auth |
| `notification-service` | 6003 | MySQL | Axum | Announcements CRUD + WebSocket push |
| `prue_backend` | 6004 | SQLite/MySQL | Axum | pure-admin menu config + dynamic routes |
| `audit-service` | 6005 | MySQL | Axum | Operation + login logs, retention, compliance export |
| `config-service` | 6006 | MySQL | Axum | Namespaces, items, watch notifications |
| `file-service` | 6007 | MySQL | Axum | File metadata, presigned upload, download |

Each Rust service has:
- `Cargo.toml` with `[[bin]]` for `dump-openapi` (utoipa generator)
- `src/{lib,main,openapi,error,config}.rs` + `routes/` + `services/` (or `service/`)
- `migrations/{mysql,sqlite}/V{NNN}__*.sql` (legacy flat `migrations/` for iam / token)
- `AGENTS.md` declaring service-specific conventions, endpoint domain, env vars, exemptions
- `tests/*.rs` integration tests
- `Dockerfile`

---

## 3. Architecture

```
                          ┌──────────────────┐
   browser / SDK ────────►│   api-gateway    │──── Consul service discovery
                          │   :3000          │──── Redis blacklist
                          │                  │──── MySQL (read token / user)
                          └────────┬─────────┘
                ┌──────────────────┼──────────────────────┐
                ▼                  ▼                      ▼
       token-service         iam-service        notification-service
       :6001                 :6002              :6003
                │                  │                      │
                ▼                  ▼                      ▼
              MySQL              MySQL                 MySQL
                                              + WebSocket ws://…/notification/ws
                ┌──────────────────┼──────────────────────┐
                ▼                  ▼                      ▼
         prue_backend       audit-service        config-service / file-service
         :6004              :6005                :6006 / :6007
                │                  │                      │
                ▼                  ▼                      ▼
           SQLite/MySQL          MySQL                  MySQL
```

**Layer responsibilities**:
- **gateway**: JWT RS256 verify, blacklist check, expiry check, downstream routing. NO business logic.
- **token-service**: only sign/refresh/revoke. Issues `access_token` + `refresh_token`.
- **iam-service**: identity + RBAC + dept tree. Provides `Principal` extractor via `libs/jwt-ctx`.
- **business services**: each owns its bounded context. Never call each other directly — go through gateway.

---

## 4. ADR Index

| ADR | Title | Key Lesson |
|-----|-------|-----------|
| ADR-001 | api-gateway multi-file router config | Split `routes/*.router.toml` per service instead of one monolith file. Easier to diff, easier to merge. |
| ADR-002 | Service Standard Baseline | 8-item standard + exemptions + 3 CI gates. New services copy `services/agents-template.md`. |

---

## 5. Spec Index (current quarter)

| Topic | File |
|-------|------|
| Service standard sweep (P1.3) | `2026-06-25-service-standard-sweep-design.md` |
| prue_backend concurrency (P1.1+P1.2) | `2026-06-25-prue-backend-concurrency-design.md` |
| Swagger utoipa coverage (P2.1) | `2026-06-25-swagger-coverage-design.md` |
| OpenAPI auto-dump (P2.2) | `2026-06-25-openapi-auto-dump-design.md` |
| Coder skills (P2.3) | `2026-06-25-coder-skills-design.md` |
| L4 triggers (P2.4) | `2026-06-25-l4-triggers-design.md` |
| Frontend verification (P2.5) | `2026-06-25-p25-verification.md` |
| E2E stability (P3.3) | `2026-06-25-e2e-stability-design.md` |
| Knowledge consolidation (P3.5) | `2026-06-25-knowledge-consolidation-design.md` |

Older specs live in the same `docs/superpowers/specs/` directory and describe services that are already shipped.

---

## 6. Skill Catalog

### Project skills (`.agents/skills/`)

| Skill | Purpose |
|-------|---------|
| `coder-add-route` | Scaffold new route handler with utoipa annotations + openapi.rs registration |
| `coder-migrate-db` | Scaffold dual MySQL/SQLite migration files with up/down |

### Platform skills (`.trae/skills/`)

| Skill | Purpose |
|-------|---------|
| `coder-base-images-sync` | Sync base Docker images to internal registry |
| `coder-dev-bootstrap` | Set up local dev environment |
| `coder-docker-pull` | Pull latest Docker images |
| `coder-e2e-test` | Run e2e tests against infrastructure |
| `coder-lib-upgrade` | Upgrade shared Rust libraries |
| `coder-service-audit` | Audit services against AGENTS.md standards |
| `plan-eng-review` | Eng-manager-mode plan review |

### How skills trigger

See `.trae/rules/project_rules.md` for path-based triggers. AI loads skills based on what files the user is touching.

---

## 7. Known Pitfalls

These are real issues we've hit. Save yourself an afternoon:

1. **`utoipa::OpenApi::to_yaml()` requires the `yaml` feature** — `Cargo.toml` MUST have `utoipa = { version = "5", features = ["axum_extras", "chrono", "yaml"] }`. Without it, `ApiDoc::openapi().to_yaml()` won't compile.

2. **Local `cargo` won't compile** because of missing `cc` linker. Use `node scripts/run-tests-docker.js <svc>` or build inside `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl` container.

3. **`api-gateway` blacklist test fails pre-existing** — it requires host Redis at `127.0.0.1:6379`. Unrelated to any change in flight. Run via Docker (uses test Redis) and it passes.

4. **Hand-written `openapi.yaml` is gone** — all 4 services now generate from utoipa. If you need to change the spec, edit the code (`#[utoipa::path]`, `ApiDoc::openapi()`), then `scripts/dump-openapi-specs.sh`. Don't edit the yaml directly.

5. **Migrations are forward-only** — never edit a shipped migration. Add a new `V{N+1}__*.sql`. The CI spec check enforces both MySQL and SQLite versions for prue_backend.

6. **JWT context uses `Authorization` header only** — see `.trae/rules/auth-rules.md`. Do NOT read `X-User-*` headers anywhere. Use `libs/jwt-ctx::Principal` extractor.

7. **Cargo.lock changes whenever utoipa features flip** — that's expected. Commit it together with the Cargo.toml change.

8. **`check-spec.js` complains about missing `summary` fields** — that's a warning, not an error. The check exits 0 even with warnings. Add summaries only if you care about UX (utoipa doesn't generate them by default).

9. **`scripts/contracts-gen.js --check` regenerates `libs/common-error/src/codes.rs`** — never edit that file directly. Edit `contracts/error-codes.yaml` instead, then re-run.

10. **WS auth on `notification/ws`** — gateway validates JWT and injects user identity. Browser auto-carries `Authorization` on WS upgrade. Don't pass token in URL.

---

## 8. Common Commands

```bash
# CI gates (run before push)
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check

# Cargo tests (docker)
node scripts/run-tests-docker.js audit-service config-service file-service prue_backend

# Regenerate OpenAPI yaml (only when handler annotations changed)
scripts/dump-openapi-specs.sh

# E2E
cd tests/e2e && pnpm install
pnpm -C tests/e2e test:smoke        # ~2 min smoke subset
pnpm -C tests/e2e test:api          # full API
pnpm -C tests/e2e test:ui           # full UI (needs frontend running)

# Build images
node scripts/base-images-sync.js --check

# Observability overlay (Prometheus + Grafana + consul-template)
docker compose -f infrastructure/docker-compose.yml up -d
scripts/start-observability.sh
# Then: Prometheus :9090, Grafana :3001 (admin/admin)

# Install pre-commit hook (one-time)
pre-commit install
# OR fallback:
cp scripts/git-hooks/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
```

---

## 9. CI Gates

| Gate | Script | Trigger |
|------|--------|---------|
| Service Standard | `node scripts/check-service-standard.js` | `services/**` |
| Spec Check | `node scripts/check-spec.js` | `services/*/openapi.yaml` |
| Contracts Check | `node scripts/contracts-gen.js --check` | `contracts/**` |
| OpenAPI Drift | `scripts/dump-openapi-specs.sh` + `git diff --exit-code` | `services/*/openapi.yaml` |
| Rust Tests | `node scripts/run-tests-docker.js <svc>` | `services/<svc>/**` |
| E2E Smoke | `pnpm -C tests/e2e test:smoke` | pre-deploy |

All gates run in GitLab CI `test` stage; failure blocks build/deploy.

---

## 10. Navigation Map

- **Need to add a route?** → `.agents/skills/coder-add-route/SKILL.md`
- **Need to add a migration?** → `.agents/skills/coder-migrate-db/SKILL.md`
- **Need to change contract?** → Edit `contracts/*.yaml`, run `scripts/contracts-gen.js`
- **Need to review a plan?** → `.trae/skills/plan-eng-review/SKILL.md`
- **Need to audit a service?** → `.trae/skills/coder-service-audit/SKILL.md`
- **Need to understand an old decision?** → `docs/decisions/ADR-*.md`
- **Need per-service conventions?** → `services/<svc>/AGENTS.md`
- **Need to know what's coming?** → `docs/superpowers/plans/2026-06-25-overall-roadmap.md`

---

_Last updated: 2026-06-25 (P3.5 close). When adding a new service / skill / ADR, update the relevant section here._