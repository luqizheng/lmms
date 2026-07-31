# Service-to-Service Authentication (Phase 5-B)

> **Status**: Active since 2026-07-07 (phase5b). Supersedes Consul Connect mTLS (phase5a, deprecated).

## Overview

After phase5a-2 (dephase Consul Connect), service-to-service authn moved from mTLS sidecar to **JWT-based**:

- **User JWT**: gateway issues after login; `aud: "user"`; verified by all services via JWKS
- **Service JWT**: token-service issues via `POST /tokens/service-jwt`; `aud: "<target>"`; used for internal calls (notification→iam, token→iam)

## Architecture

```
[Startup]
  service → GET JWKS from token-service:8082/.well-known/jwks.json
  service → POST /tokens/service-jwt (with bootstrap token) → cached service-jwt

[Inbound request]
  Authorization: Bearer <user-jwt or service-jwt>
  → Principal extractor (jwt-ctx) verifies signature via JWKSCache
  → /iam/internal/* also requires aud="iam-service" (verify_aud middleware)

[Outbound call]
  ResilientClient or manual header injection
  → Authorization: Bearer <service-jwt for target>
```

## Bootstrap Tokens

Each business service has a pre-shared `SERVICE_BOOTSTRAP_TOKEN` (32-byte random). At startup, the service exchanges it for a short-lived service-jwt (1h TTL) via `POST /tokens/service-jwt`.

**Dev values** (in `infrastructure/.env`):
```bash
SERVICE_BOOTSTRAP_TOKENS=dev-bt-iam,dev-bt-token,dev-bt-notification,...
```

**Production**: use a secret manager (Vault, K8s Secret). Never commit real tokens.

**Rotation**: generate new token → update `SERVICE_BOOTSTRAP_TOKENS` env on token-service → restart token-service → update `SERVICE_BOOTSTRAP_TOKEN` on the affected service → restart that service.

## Key Components

| Component | Location | Role |
|---|---|---|
| `libs/digua_common-jwt` | JWKSCache, verify_token, sign_token, ServiceJwtCache | Crypto primitives |
| `libs/jwt-ctx` | Principal::from_token (async, verify) | Extractor for Axum |
| `libs/digua_authz` | verify_aud_principal_middleware | Route-level aud gate |
| `token-service` | POST /tokens/service-jwt | Minting endpoint |
| Each service main.rs | JWKS + ServiceJwtCache at startup | Cold-start init |

## Env Vars (per service)

| Var | Required | Default | Description |
|---|---|---|---|
| `SERVICE_BOOTSTRAP_TOKEN` | Yes (except token-service) | — | Pre-shared secret for /tokens/service-jwt |
| `JWKS_URL` | No | `http://token-service:8082/.well-known/jwks.json` | JWKS endpoint |
| `TOKEN_SERVICE_URL` | No | `http://token-service:8082` | token-service base URL |
| `TENANT_ID` | No | `1` | Default tenant for service-jwt claims |
| `SERVICE_BOOTSTRAP_TOKENS` | Yes (token-service only) | — | Comma-separated accepted tokens |

## Error Handling

| Scenario | HTTP Status | Behavior |
|---|---|---|
| Missing Authorization header | 401 | Principal extractor rejects |
| Invalid signature | 401 | verify_token returns SignatureInvalid |
| Expired JWT | 401 | verify_token returns Expired |
| Wrong audience | 401 | verify_aud middleware rejects |
| JWKS unreachable at startup | panic | Service cannot start without verification keys |
| Service-jwt refresh failure | retry w/ backoff | 1s/2s/4s/.../60s cap |

## Adding a New Internal Endpoint

1. Add route to service's router
2. Wrap with `verify_aud_principal_middleware::<AppState>("<service-name>")`
3. Handler uses `Principal` extractor for identity
4. Outbound calls inject `Authorization: Bearer <service-jwt>` via `ServiceJwtCache::get("<target>")`

```rust
let internal = internal_routes::routes(state.clone())
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        verify_aud_principal_middleware::<AppState>("my-service"),
    ));
```
