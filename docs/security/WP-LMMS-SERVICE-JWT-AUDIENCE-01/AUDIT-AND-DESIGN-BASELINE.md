# WP-LMMS-SERVICE-JWT-AUDIENCE-01 · Audit & Design Baseline

- Repository: `luqizheng/lmms`
- Audit base: `main` @ `55e379ff4cd2e26ccb2d0ac20b868e7f9a86c9e4`
- Date: 2026-11-26
- Status: **AUDIT CONFIRMED / DESIGN PROPOSAL — NOT IMPLEMENTED**
- Scope: `POST /tokens/service-jwt` bootstrap-owner → target-audience authorization.

## 1. Executive finding

**Confirmed authorization gap (P1):** LMMS authenticates the caller's Bootstrap Token and resolves its `owner`, but does not authorize that owner for the requested `aud`. The handler only rejects an empty audience and then signs the caller-supplied value into the Service JWT.

Evidence:
- `services/token-service/src/routes/service_jwt.rs`: `find_active_owner_by_token(...)` resolves the caller; `req.aud.is_empty()` is the only audience check; claims are then signed with `sub=caller` and `aud=req.aud`.
- `services/token-service/tests/service_jwt_test.rs`: `service_jwt_ct_claim_and_owner_from_db` explicitly accepts `project-manager` → `file-service`; `seed_defaults_falls_back_to_built_in_dev_tokens` accepts `iam-service` → `audit-service`.
- `services/token-service/src/models/bootstrap_token.rs`: Bootstrap Token persistence and lookup contain owner/token/status/expiry only; no audience grant lookup exists.
- MySQL and PostgreSQL `V003__bootstrap_tokens.sql`: `token_bootstrap_tokens` has owner, token, status, expiry and timestamps; no owner→audience authorization relation exists.
- `libs/authz/src/verify_aud.rs`: receiver middleware validates a JWT against one fixed expected audience via `Principal::from_token(..., Some(expected_aud))`. This is a consumer-side audience check, not issuer-side permission for a bootstrap owner to request that audience.

## 2. Existing responsibility split

This work adds only a **credential-issuance boundary**:
- LMMS authenticates Bootstrap Token possession, resolves its registered service owner, checks whether that owner may obtain a Service JWT for the requested infrastructure/service audience, signs the short-lived credential, and provides cryptographic verification primitives.
- The receiving service validates signature, expiry and exact expected audience.
- The business system remains authoritative for business-resource authorization (tenant membership, project/resource ownership, roles, actions, delegation, and business policy). LMMS must not decide these.

A grant such as `project-manager → file-service` means only that the registered caller may obtain a credential addressed to `file-service`. It does **not** authorize any business operation or resource.

## 3. Confirmed current flow

```
Authorization: Bearer <bootstrap token>
  → find_active_owner_by_token(token)
  → owner (DB row)
  → validate aud is non-empty
  → sign Service JWT { iss, sub=owner, aud=request.aud, tid, roles=[service], purpose=service, ... }
  → receiver checks signature + expiry + exact aud
```

The missing step is an issuer-side authorization check between owner resolution and signing:
`is_audience_allowed(owner, req.aud)`.

## 4. Design recommendation (proposal, not code)

Prefer an explicit, normalized owner-to-audience grant relation rather than encoding authorization in service-name conventions or accepting arbitrary non-empty strings.

Proposed logical model:
- `owner`: canonical Bootstrap Token principal.
- `audience`: exact registered receiving service identifier.
- `status`: active/disabled (or an equivalent revocation mechanism).
- audit metadata: created/updated actor and timestamps, according to existing LMMS audit conventions.
- uniqueness: one grant per canonical owner + audience.

Issuance must fail closed:
1. Resolve an active, unexpired Bootstrap Token.
2. Resolve its **canonical owner identity**.
3. Validate `aud` syntax/length against the service identifier contract.
4. Require an active exact owner→audience grant.
5. Only then sign; never silently fall back to allow-all, defaults, or a platform-wide wildcard.
6. Keep receiver-side exact audience validation unchanged.

### Important rotation/grace edge case

Bootstrap Token rotation stores a grace-period token in an owner row suffixed with `#old`. Today lookup returns that row's owner string. If grants are keyed by owner, the implementation must explicitly map a grace token to its canonical owner (or store a separate immutable principal-owner field); it must not accidentally treat `owner#old` as a distinct service principal or bypass grants. This needs a dedicated regression test.

### Bootstrap/default policy

The existing built-in development Bootstrap Tokens do not establish an authorization policy. Do not infer `owner == audience`, allow all internal services, or automatically grant every audience. Before implementation, inventory every legitimate caller→audience pair across LMMS deployment config, SDKs, and downstream repositories, then explicitly seed only those approved pairs. Unknown owners/audiences must be denied.


## 4A. Built-in LMMS startup-critical authorization (priority 0)

The first implementation slice must protect the LMMS built-in authentication/login path before adding generic service grants.

### Verified in-repository call paths

| Bootstrap owner (caller) | Requested audience | Evidence / purpose | Priority |
|---|---|---|---|
| `iam-service` | `token-service` | `services/iam-service/src/service/token_forward.rs` calls `ServiceJwtCache::get("token-service")` before `POST /tokens/user-jwt`. IAM delegates user-JWT signing to token-service. This is login/authentication critical. | **P0 — mandatory** |
| `notification-service` | `iam-service` | `services/notification-service/src/iam_client.rs` calls `ServiceJwtCache::get("iam-service")` for `/iam/internal/membership-bootstrap` and fallback membership reads. Startup projection bootstrap invokes this path before HTTP serving; failure is logged and service continues degraded. | **P1 — mandatory for notification IAM integration** |

Canonical service identity in this repository is `iam-service`, not `iam-user`. If “iam-user” refers to the IAM user/login component, its deployed service identity is still `iam-service`.

### Token-service role in the built-in chain

- `token-service` is the **audience/issuer endpoint** for the IAM login flow: IAM must be allowed to obtain `aud=token-service` in order to request user-JWT issuance.
- The built-in code inspected does not show token-service using `ServiceJwtCache` to call another service. Its own Bootstrap Token row is currently seeded, but that alone is not evidence for an outbound owner→audience grant.
- Do not create `token-service → token-service` or `token-service → *` grants merely because a `token-service` Bootstrap Token exists.
- The token-service's receiver-side routes that require `aud=token-service` continue validating the exact audience; issuer-side grants are a separate gate.

### Startup seeding requirement

The P0 implementation must not depend on a human manually creating the first grants through an authenticated admin API. That would create a bootstrap deadlock: the IAM login path needs a Service JWT before the user can log in and reach the Web UI that manages grants.

Required behavior:
1. Add the built-in grants as code-owned, version-controlled LMMS system defaults, separate from the Bootstrap Token secret values.
2. During token-service startup, after schema migrations and before the HTTP listener accepts requests, idempotently ensure the mandatory built-in grant rows exist.
3. Seed these grants even when production Bootstrap Token secrets are supplied through `SERVICE_BOOTSTRAP_TOKENS` / `SERVICE_BOOTSTRAP_OWNERS`; secret injection must not suppress system authorization bootstrap.
4. Verify that each required grant's owner has a corresponding active Bootstrap Token principal. If required owner configuration is absent or inconsistent, fail startup with a clear non-secret diagnostic; never fall back to a dev secret in production.
5. Treat these rows as system-managed grants, not ordinary user-created grants. Generic admin operations must not accidentally delete/disable mandatory login-path grants. Changes require an explicit versioned code/config change and review.
6. Startup seeding must be transactional/idempotent and safe under concurrent token-service replicas. Use dialect-compatible migrations and uniqueness constraints for both MySQL and PostgreSQL.
7. Do not log Bootstrap Token values. Do log grant identifiers and seed/validation outcomes.

**Important:** “ensure grants exist on every startup” means repair missing system-default rows idempotently, not silently restore arbitrary manually revoked custom grants. The mandatory built-in grants should be explicitly marked/system-owned so their lifecycle is unambiguous.

### Required first-slice tests

- Fresh empty DB: migrations + built-in Bootstrap Token seeding + built-in grant seeding completes before listener startup.
- Production-style custom secrets with canonical owners: built-in grants are still created; no dev token is inserted.
- IAM can obtain `aud=token-service` and complete the token-service user-JWT call.
- IAM cannot obtain an unrelated audience (e.g. `file-service`) without a separate grant.
- Notification can obtain `aud=iam-service` and complete membership bootstrap.
- Notification cannot obtain unrelated audiences.
- Missing required built-in owner or DB failure prevents token-service readiness/startup; no permissive fallback.
- Concurrent token-service startup does not duplicate grants or fail due to seed races.
- Built-in grants remain present after restart and after Bootstrap Token rotation; the old grace token resolves to the same canonical owner grant.
- System-managed built-in grants cannot be disabled/deleted through generic admin endpoints.


## 5. Required audit/design closure before implementation

- [ ] Inventory all callers of `POST /tokens/service-jwt` and their requested audiences across LMMS, ai-pm, smark-artifact-service and smark-sem.
- [ ] Produce an approved owner→audience matrix with evidence and environment distinctions (dev/test/prod).
- [ ] Decide grant lifecycle: create, disable, delete, audit, and whether grants are managed via existing bootstrap-token admin routes or a distinct admin surface.
- [ ] Decide whether bootstrap token rotation preserves grants (recommended: yes, because grants belong to canonical owner, not secret material).
- [ ] Decide treatment of legacy installations with no grants. Recommended safe behavior: fail closed; provide an explicit migration/seed procedure before enabling enforcement.
- [ ] Confirm database dialect support and migration strategy for both MySQL and PostgreSQL.
- [ ] Review whether the owner naming/identity model is stable enough to be a grant key; do not silently normalize arbitrary owner strings.
- [ ] Review rate limiting and audit logging for denied issuance attempts without logging bearer secrets.

## 6. Required tests

At minimum:
1. Active grant: allowed owner→audience issues JWT.
2. Missing grant: denied; no JWT emitted.
3. Disabled grant: denied.
4. Same Bootstrap Token cannot request a different audience.
5. Wrong/unknown audience denied.
6. Inactive/expired Bootstrap Token denied before grant use.
7. Grace-period old token maps to the canonical owner and follows the same grant set.
8. Rotated token retains the owner's grants.
9. Deleting/disabling a grant takes effect according to the documented consistency model.
10. Receiver-side exact audience rejection remains intact.
11. Cross-tenant `tid` and `ct` behavior is unchanged; audience grants do not become tenant/business authorization.
12. DB failure is fail-closed (5xx, never sign).

## 7. Explicit non-goals / constraints

- Do not change Refresh Token work or its claims/compatibility policy.
- Do not redesign Resource Token claims or its consumer contract.
- Do not change Service JWT claims (`iss/sub/aud/tid/ct/roles/purpose/exp`) unless a separately reviewed contract change is required.
- Do not make LMMS the authority for business resource authorization.
- Do not weaken receiver-side audience validation.
- Do not add wildcard grants or implicit owner==audience behavior without an explicit, separately reviewed decision.
- Do not merge this work or deploy schema/code as part of this audit branch.

## 8. Preliminary disposition

The issue is **confirmed**. A dedicated owner→audience grant is the recommended authorization mechanism, but the exact approved grant matrix and operational lifecycle must be finalized before enforcement. This branch contains audit/design documentation only; no application code, migration, SDK, or runtime behavior has been changed.
