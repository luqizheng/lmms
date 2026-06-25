# P2.5b OpenAPI Coverage for the Remaining 4 Services

## Context

P2.1 (commit `23a85aa`) added full utoipa annotations to 4 services:
`audit-service`, `config-service`, `file-service`, `prue_backend`.
The other 4 services — `token-service`, `iam-service`, `notification-service`,
`api-gateway` — have partial utoipa coverage and no `openapi.yaml`.

This task brings all 8 services to 100% openapi.yaml coverage so:
- All 8 services expose Swagger UI at `/swagger-ui` (when SWAGGER_ENABLED=true)
- `scripts/dump-openapi-specs.sh` generates all 8 yaml files
- `check-spec.js` validates all 8

## Decision

For each of the 4 remaining services:

1. **Add utoipa::path annotations on every public handler** (where missing)
2. **Add utoipa::ToSchema on every request/response DTO**
3. **Move ApiDoc to a dedicated `src/openapi.rs` file** (matches audit/config/file pattern)
4. **Add `[[bin]] dump-openapi` to Cargo.toml + `src/bin/dump-openapi.rs`** (same template)
5. **Generate `openapi.yaml` via `cargo run --bin dump-openapi`**

Then:
- `scripts/dump-openapi-specs.sh` updated to dump all 8 services
- `check-spec.js` will validate all 8 (no change needed — it globs `services/*/openapi.yaml`)

## Current state per service

| Service | utoipa::path count | openapi.rs | dump-openapi | openapi.yaml |
|---------|-------------------|------------|--------------|--------------|
| audit-service | 12 | ✓ | ✓ | ✓ |
| config-service | 13 | ✓ | ✓ | ✓ |
| file-service | 6 | ✓ | ✓ | ✓ |
| prue_backend | 8 | ✓ | ✓ | ✓ |
| **token-service** | 9 | ✓ | ✗ | ✗ |
| **iam-service** | 11 | ✓ | ✗ | ✗ |
| **api-gateway** | 0 | inline | ✗ | ✗ |
| **notification-service** | 7 | inline | ✗ | ✗ |

## Why the 4 services lack dump-openapi

The pattern (`src/bin/dump-openapi.rs` + `Cargo.toml [[bin]]`) was established in
P2.1 for the 4 services that needed full annotation retrofit. The other 4
were not retrofitted because they had `SwaggerUi` already wired (just inline
`ApiDoc` definitions). Adding the dump-openapi binary is mechanical:
- Copy the binary template
- Add `[[bin]]` line
- Run it once to produce `openapi.yaml`

For utoipa annotation gaps, see § "annotation gaps" below.

## Annotation gaps

### token-service (9 → 12 expected)
Routes that lack `#[utoipa::path]`:
- `routes/jwks.rs::handle_jwks` (no annotation — read-only, no params)
- `routes/api_key.rs::handle_create_api_key` (might be missing)
- `routes/user_jwt.rs::handle_user_jwt` (might be missing)

### iam-service (11 → ~25 expected)
Routes file structure: `auth/users/roles/permissions/org_units` — many
endpoints. Need to verify each handler has utoipa::path with request_body
+ responses + tag.

### api-gateway (0 → N expected)
Gateway is special: it has a single catch-all `/api/v1/*` proxy. The
OpenAPI representation is the AGGREGATE of all downstream services. Options:
- A. List each known path as a passthrough `#[utoipa::path(..., responses(...))]`
- B. Skip api-gateway swagger (gateway has no own routes beyond /health/metrics)
- C. Forward `/api-docs/openapi.json` to each downstream and merge

**Choice: B**. api-gateway stays Swagger-less. It's a passthrough; the
Swagger UI is on each downstream service (4 already have it; adding
token/iam/notification gives 7 of 8 services with their own Swagger UI).
api-gateway gets only the placeholder ApiDoc for `/health` and `/metrics`.

### notification-service (7 → 9 expected)
Has ApiDoc in main.rs (not separate file). 7 utoipa::path in rest.rs.
Missing probably: `unread-count` and `mine` (list) routes. Need to verify.

## Files to touch

| File | Change |
|------|--------|
| `services/token-service/Cargo.toml` | add `[[bin]]` for dump-openapi, add utoipa + yaml features if missing |
| `services/token-service/src/bin/dump-openapi.rs` | NEW (copy from audit-service) |
| `services/token-service/src/openapi.rs` | ensure all paths included |
| `services/token-service/openapi.yaml` | NEW (generated) |
| `services/iam-service/Cargo.toml` | same as token |
| `services/iam-service/src/bin/dump-openapi.rs` | NEW |
| `services/iam-service/src/openapi.rs` | ensure all paths included |
| `services/iam-service/openapi.yaml` | NEW (generated) |
| `services/notification-service/Cargo.toml` | same |
| `services/notification-service/src/bin/dump-openapi.rs` | NEW |
| `services/notification-service/src/openapi.rs` | NEW (extract from main.rs) |
| `services/notification-service/openapi.yaml` | NEW (generated) |
| `services/api-gateway/Cargo.toml` | same |
| `services/api-gateway/src/bin/dump-openapi.rs` | NEW |
| `services/api-gateway/src/openapi.rs` | NEW (extract from main.rs) |
| `services/api-gateway/openapi.yaml` | NEW (generated) |
| `scripts/dump-openapi-specs.sh` | add the 4 new services |
| `docs/superpowers/specs/2026-06-25-p25b-remaining-utoipa-design.md` | NEW (this doc) |
| `docs/USER_MANUAL.md` | update § Swagger UI table with all 8 services |
| `docs/ai-context.md` | update § 1 / § 9 |

## Verification

1. `cargo check -p token-service -p iam-service -p api-gateway -p notification-service` all exit 0
2. `scripts/dump-openapi-specs.sh` produces all 8 openapi.yaml
3. `node scripts/check-spec.js` exit 0
4. SWAGGER_ENABLED=true + start service → curl `/api-docs/openapi.json` returns valid spec
5. UI shows all handlers in the spec

## Out of scope

- Cross-service API aggregation in api-gateway (option C above) — future
- Hand-written openapi.yaml for any service — auto-generated only
- OTel integration with Swagger UI (e.g. /swagger-ui with trace_id propagation)