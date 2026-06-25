# P3.2 Gateway Rate Limiting Design

## Decision

**Adopt recommended path: Gateway-central rate limiting.**

## Context

No rate limiting currently. Goals:
- Anti-DDoS / anti-brute-force at ingress
- Per-user / per-IP quotas for fair use
- Zero service-code changes (8 services untouched)
- Observable: rejected requests must surface in `/metrics`

## Implementation

### Rate Limiter Type: Token Bucket per Key

- Algorithm: `governor` crate (Rust-native, lock-free, well-tested)
- Bucket per `(route, identity_key)` tuple
- Identity key priority:
  1. `X-User-Id` (extracted by gateway after JWT validation) — preferred for authenticated routes
  2. `X-Real-IP` (injected by nginx) — for unauthenticated routes
  3. Fallback: client IP from socket peer

### Configuration

`infrastructure/config/api-gateway/rate-limits.toml`:

```toml
[defaults]
requests_per_second = 100
burst = 200

[routes."/v1/auth/login"]
requests_per_second = 5
burst = 10

[routes."/v1/notification/ws"]
requests_per_second = 2
burst = 5

[routes."/v1/iam/auth/password"]
requests_per_second = 3
burst = 5
```

### Response

- HTTP 429 with `Retry-After: 1` header
- Body: `{"code":"RATE_LIMITED","message":"too many requests","retry_after_ms":1000}`
- Recorded in Prometheus: `gateway_rate_limited_total{route="..."}`

### Middleware Order (api-gateway)

```
Request
  ↓
[JWT validation]  ← existing
  ↓
[Rate limit]      ← NEW
  ↓
[Blacklist check] ← existing
  ↓
[Route resolve]
  ↓
Downstream
```

Rate limit comes AFTER JWT so we can use user_id, but BEFORE blacklist so brute-force attempts don't waste blacklist lookups. (Blacklist on rate-limited requests is unnecessary cost.)

### Identity Key Extraction

- After JWT middleware sets `UserId` in request extensions
- Rate-limit middleware reads it; falls back to IP if not set
- Tests don't need to mock JWT — just verify IP-based limit works

## Files Touched

| File | Change |
|------|--------|
| `services/api-gateway/Cargo.toml` | Add `governor = "0.6"` |
| `services/api-gateway/src/middleware/rate_limit.rs` | NEW module |
| `services/api-gateway/src/middleware/mod.rs` | Register middleware |
| `services/api-gateway/src/main.rs` | Wire middleware in pipeline |
| `services/api-gateway/src/config.rs` | Load rate-limits.toml |
| `infrastructure/config/api-gateway/rate-limits.toml` | NEW config |
| `infrastructure/docker-compose.yml` | Mount rate-limits.toml into gateway |
| `services/api-gateway/tests/rate_limit_test.rs` | NEW integration test |
| `docs/superpowers/specs/2026-06-25-rate-limiting-design.md` | NEW (this doc) |

## Verification

1. Run e2e: hammer `/v1/auth/login` with 100 bad-password attempts → first ~10 succeed (with 401), rest get 429
2. Inspect `/metrics` on gateway → `gateway_rate_limited_total{route="/v1/auth/login"}` > 0
3. Unrelated routes unaffected (per-route buckets)
4. Add new entry to `rate-limits.toml`, restart gateway → new limit active