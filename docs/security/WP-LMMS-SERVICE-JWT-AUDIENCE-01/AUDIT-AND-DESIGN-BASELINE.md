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
