# token-service JWT Signing Key Persistence & Rotation — Design

Date: 2026-11-23
Status: implemented
Scope: services/token-service, examples/docker-deploy*, docs

## 1. Problem (root cause)

1. `main.rs::load_or_generate_keys` generated a fresh RSA key pair whenever the
   key files were missing **or unreadable** — including the "private key exists
   but is corrupt" case (fail-open, silently overwriting the old key with
   non-atomic `fs::write`).
2. Compose files (dev + full/prod) mount **no volume** for token-service, so
   `/app/keys` lived in the container layer. Any `docker compose up` after a
   container recreation regenerated the key → every previously issued JWT
   failed verification → all users logged out, service-to-service auth broken.
3. `kid` was hard-coded `"default"` (`issuer.rs::JWT_KEY_ID`); single key, no
   rotation semantics.
4. `services/token-service/keys/private.pem` was committed to git (dev-only
   key, but a leaked private key must be treated as compromised and rotated).

## 2. Design: single-file keystore

One source of truth: `keystore.json` (path from `TOKEN_JWT_KEYSTORE_PATH`,
default = `keystore.json` in the same directory as
`TOKEN_JWT_PRIVATE_KEY_PATH` → `/app/keys/keystore.json` in containers).

```json
{
  "version": 1,
  "active_kid": "<32 hex chars>",
  "keys": [
    {
      "kid": "…",
      "status": "active",          // active | retiring | retired
      "created_at": 1764…,          // unix sec
      "retired_at": null,
      "private_pem": "-----BEGIN PRIVATE KEY…",
      "public_pem":  "-----BEGIN PUBLIC KEY…"
    }
  ]
}
```

- **kid derivation**: `hex(SHA-256(RS256PublicKey DER)[0..16])` — content
  addressed, stable across restarts, unique per key.
- **Legacy compatibility**: keys imported from the old
  `private.pem`/`public.pem` pair keep `kid = "default"` so already-issued
  tokens (header `kid:"default"`) keep verifying during migration.
- **Atomic write**: write `keystore.json.tmp` → fsync → rename. A crash can
  never leave a half-written keystore.
- **Permissions**: file `0600`, directory `0700` (unix; enforced on load —
  group/other-readable keystore is a fail-closed startup error).
- **Logging**: kid / status / count only. Never log PEM content.

## 3. Fail-closed startup matrix (`KeyStore::init`)

| Disk state | Behaviour |
|---|---|
| keystore.json valid, all PEMs parse, perms ok | load |
| keystore.json corrupt (bad JSON / bad PEM) | **exit 1** — never regenerate |
| keystore.json perms too wide (unix) | **exit 1** |
| keystore.json absent, old PEM pair present | import (kid=`default`) → atomic save |
| keystore.json absent, exactly one old PEM present | **exit 1** (never overwrite) |
| old PEM present but unparseable | **exit 1** (was: silent overwrite) |
| nothing present | generate fresh key pair (kid = fingerprint), atomic save |

## 4. Rotation state machine

```
active ──rotate──▶ retiring ──retire──▶ retired ──delete(≥ min_retention)──▶ gone
 (signs)            (verify only)       (verify-only, not in JWKS)
```

- `rotate`: generate new key → becomes `active`; previous active → `retiring`.
  Old tokens keep verifying (key stays in JWKS). No forced logout.
- JWKS publishes `active + retiring` keys (each with its own kid).
- `retire`: retiring → retired (removed from JWKS). Operator confirms no
  unexpired tokens remain signed by that kid.
- `delete`: only `retired` keys with `now - retired_at ≥ TOKEN_JWT_MIN_RETENTION_SEC`
  (default 2 592 000 = 30 d = default refresh TTL). Otherwise HTTP 409.
- Same operations available offline via `keytool` subcommands (rotate /
  retire / delete / inspect / backup / restore) for deployments that prefer
  stopping the service.

## 5. Verification continuity

- `TokenIssuer` signs with the active key pair (kid stamped into JWT header).
- Refresh-token decode tries every stored public key (active → retiring), so
  refresh families issued before a rotation keep working.
- Service JWTs (`mint_service_bearer`) use the active private key.
- Downstream verifiers (`common-jwt::JWKSCache`, api-gateway `GW_JWKS_URL`)
  already support multi-kid caches with periodic refresh — no downstream
  change required.

## 6. Admin endpoints (service-jwt `aud=token-service`)

| Endpoint | Method | Notes |
|---|---|---|
| /admin/keys | GET | list kid/status/timestamps (never PEMs) |
| /admin/keys/rotate | POST | rotate; returns new + previous kid |
| /admin/keys/{kid}/retire | POST | retiring → retired |
| /admin/keys/{kid} | DELETE | retired + retention satisfied, else 409 |

## 7. Backup / restore

- `keytool backup --keystore <p> --out <f>` (backup file 0600, must stay out
  of git — *.backup.json ignored).
- `keytool restore --backup <f> --keystore <p>` validates JSON + every PEM
  before atomically replacing the live keystore.
- Restore then restart → previous JWTs verify (tested in unit tests).

## 8. Deployment changes

- dev + full compose units: `token-keys:` named volume mounted at `/app/keys`.
- `.image.env` untouched (image only). No new required env for existing
  deployments: without `TOKEN_JWT_KEYSTORE_PATH` the service auto-imports the
  legacy PEM pair on first boot after upgrade — zero-breakage migration.
- Leaked `services/token-service/keys/*.pem` removed from git (dev key;
  production keys are generated in-volume on first boot or restored from an
  operator-held backup).

## 9. Test matrix (unit, no DB required)

1. first boot generates keystore · 2. second boot loads same key ·
3. re-init idempotent (container recreation) · 4. JWT survives restart ·
5. JWKS exposes correct n/e for active kid · 6. old+new tokens verify after
rotate · 7. retired key leaves JWKS / removed key rejects old tokens ·
8. corrupt keystore fails closed · 9. permissive perms fail closed (unix,
runs in CI) · 10. backup→restore→old JWT verifies. Plus: half-missing legacy
pair fails closed, legacy pair import keeps kid `default`.
