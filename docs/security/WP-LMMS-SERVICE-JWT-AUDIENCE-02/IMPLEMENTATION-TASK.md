# WP-LMMS-SERVICE-JWT-AUDIENCE-02 · Built-in Service Authorization Bootstrap

- Repository: `luqizheng/lmms`
- Base: `main` @ `55e379ff4cd2e26ccb2d0ac20b868e7f9a86c9e4`
- Branch: `feat/wp-lmms-service-jwt-audience-02-builtin-bootstrap`
- Status: IMPLEMENTATION TASK / DRAFT
- Depends on: WP-LMMS-SERVICE-JWT-AUDIENCE-01 audit baseline
- Scope: built-in LMMS service identities and startup-critical owner→audience grants only.

## 1. Objective

Implement owner→audience authorization for Service JWT issuance without creating a bootstrap deadlock. A fresh or restarted LMMS deployment must have its mandatory built-in service identities and system-managed grants ready before token-service accepts issuance requests.

This phase is intentionally narrower than generic audience-grant administration. Do not add a UI or broad CRUD API.

## 2. Confirmed built-in call graph and initial authorization matrix

| Bootstrap principal / caller | Audience | Reason | Classification |
|---|---|---|---|
| `iam-service` | `token-service` | IAM obtains a Service JWT before calling `POST /tokens/user-jwt`; required for user login/token issuance. | Mandatory system grant, P0 |
| `notification-service` | `iam-service` | Notification obtains a Service JWT for IAM internal membership bootstrap/read paths. | Mandatory system grant, P1 |

Canonical IAM service identifier in this repository is `iam-service`. “iam-user” refers to the IAM user/login capability, not a separate audience unless code/config inventory proves otherwise.

### Identity existence vs outbound grant

The built-in principals `iam-service`, `token-service`, and `notification-service` must be represented by valid Bootstrap Token owner configuration in every deployment that enables those services.

However, the inspected in-repository call graph proves only the two outbound grants above:
- `iam-service → token-service`
- `notification-service → iam-service`

Do **not** invent `token-service → token-service`, `token-service → *`, or blanket mutual grants. A Bootstrap Token identity does not itself imply permission to obtain a Service JWT for every audience. Before coding, search all built-in callers and add any additional edge only with a concrete call-site/configuration reference.

## 3. Mandatory implementation requirements

### 3.1 Grant persistence

Implement a normalized owner→audience grant persistence model with:
- canonical owner/principal identity;
- exact audience identifier;
- active/system-managed state (or equivalent immutable system-default classification);
- creation/update timestamps consistent with LMMS conventions;
- unique constraint on canonical owner + audience.

Support every currently supported database dialect, at minimum MySQL and PostgreSQL. Preserve existing test-dialect compatibility.

### 3.2 Fail-closed issuance

In `POST /tokens/service-jwt`:
1. Authenticate the Bootstrap Token as today.
2. Resolve the canonical owner principal.
3. Validate audience syntax/length.
4. Require an active exact owner→audience grant.
5. Only then construct/sign the Service JWT.

Unknown owners, unknown audiences, absent/disabled grants, and database errors must never result in a signed token. Do not add wildcard semantics or implicit `owner == audience` authorization.

Keep receiver-side exact-audience verification unchanged. Keep Service JWT claims unchanged unless a separate reviewed contract change proves necessary.

### 3.3 Built-in startup bootstrap

Mandatory system grants must be provisioned automatically by token-service, without requiring a human to log in or call an admin API first.

Required ordering:
1. Load and validate configuration.
2. Connect to database.
3. Apply schema migrations.
4. Initialize/resolve Bootstrap Token principals using the existing production-safe mechanism.
5. Idempotently ensure version-controlled mandatory system grants.
6. Validate required principals and grants.
7. Initialize remaining signing/runtime dependencies as required by existing architecture.
8. Bind/serve the HTTP listener only after all mandatory authorization prerequisites succeed.

Adapt exact order to existing `main.rs`, preserving the invariant: no listener readiness before mandatory grants are ensured.

The system grant seed must run regardless of whether Bootstrap Token secrets come from built-in development defaults or production environment/config injection. Do not couple grant seeding to the development-token seeding branch.

### 3.4 System-managed lifecycle

- Built-in grants are code-owned, version-controlled defaults.
- Re-running startup repairs missing mandatory system rows idempotently.
- The seed must not overwrite or reactivate arbitrary custom grants.
- Generic admin endpoints must not delete/disable mandatory system grants.
- Changes to mandatory grants require a reviewed code/config change.
- Concurrent token-service replicas must not race into duplicate rows or startup failure.
- Use transactions/upserts/unique constraints appropriate to MySQL and PostgreSQL.
- Never log Bootstrap Token secrets.

### 3.5 Bootstrap Token rotation

Bootstrap Token rotation and grace-period old tokens must resolve to the same canonical principal and therefore the same grant set. Do not use the suffixed `owner#old` row identity as a distinct authorization principal. Add regression coverage for current and grace tokens.

### 3.6 Production configuration safety

Audit the existing `SERVICE_BOOTSTRAP_TOKENS` / `SERVICE_BOOTSTRAP_OWNERS` handling and all deployment examples before implementation.

- Ensure the three built-in principals required by enabled built-in services are configured consistently.
- Do not silently inject development secrets in production.
- If a mandatory principal is absent or inconsistent, fail token-service readiness/startup with a clear diagnostic containing no secret material.
- Document exact production configuration requirements and upgrade procedure for existing deployments.
- Do not assume every deployment enables Notification Service; define whether its principal/grant is conditionally mandatory based on enabled service topology, and make that decision explicit and testable.

## 4. Required test matrix

### Database and startup
1. Fresh MySQL database: migrations + principal initialization + mandatory grant seed succeed before listener readiness.
2. Fresh PostgreSQL database: same.
3. Existing database with missing mandatory grant: startup repairs it.
4. Existing database with existing mandatory grant: startup is idempotent.
5. Production-style injected secrets/owners: system grants are still seeded; development secrets are not inserted.
6. Missing required owner config: startup fails/readiness remains false with safe diagnostics.
7. Concurrent token-service replicas: no duplicate grants or transient bootstrap race.
8. Database outage/seed failure: token-service does not serve issuance requests.

### Issuance and negative authorization
9. `iam-service` can obtain `aud=token-service`.
10. `iam-service` cannot obtain unrelated `aud=file-service` without a separate explicit grant.
11. `notification-service` can obtain `aud=iam-service` when Notification Service is enabled.
12. `notification-service` cannot obtain unrelated audiences.
13. `token-service` Bootstrap Token identity exists, but no implicit outbound grant is inferred.
14. Unknown owner and unknown audience are denied.
15. Grant-store DB failure returns an error and never signs a token.

### End-to-end and regression
16. IAM login/User JWT flow completes through token-service using the built-in grant.
17. Notification membership bootstrap obtains the IAM Service JWT and succeeds.
18. Current and grace-period Bootstrap Tokens map to the same canonical owner grants.
19. Bootstrap Token rotation does not lose grants.
20. Mandatory system grants cannot be disabled/deleted through generic admin paths.
21. Receiver-side exact audience validation remains unchanged.
22. Existing Service JWT claims and `tid`/`ct` behavior remain unchanged.

## 5. Explicit non-goals

- No Service JWT audience grant UI in this phase.
- No blanket allow-all, wildcard, or owner==audience fallback.
- No business-resource authorization decisions in LMMS.
- No changes to Refresh Token work.
- No Resource Token claims/contract changes.
- No Service JWT claim changes unless separately reviewed.
- No downstream AI-PM/SEM/Artifact Service grant matrix in this phase.
- No merge or deployment as part of implementation.

## 6. Required implementation workflow

1. Rebase/branch from latest main and verify WP-01 audit baseline remains accurate.
2. Inspect current token-service startup, migration runner, Bootstrap Token seed path, rotation model, and all built-in ServiceJwtCache call sites.
3. Implement schema/model/repository and startup seed.
4. Add fail-closed issuance enforcement.
5. Add MySQL/PostgreSQL migrations and tests.
6. Run formatting, lint, unit/integration tests, dialect matrix, OpenAPI checks, and relevant CI.
7. Open a Draft PR with full delivery report and exact base/head SHAs.
8. Do not merge. Request independent security review.

## 7. Acceptance criteria

Complete only when:
- a fresh deployment initializes all enabled built-in principals and mandatory system grants without a logged-in administrator;
- IAM login does not depend on a manually provisioned audience grant;
- Notification IAM integration has its required grant when enabled;
- missing/invalid grants fail closed;
- token-service itself has a valid Bootstrap Token identity but no invented self-grant;
- MySQL and PostgreSQL migrations/tests pass;
- rotation, restart, concurrency, and negative authorization tests pass;
- claims and business authorization boundaries remain unchanged.
