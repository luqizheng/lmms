# P3.2b Service-Side Rate Limiting Design

## Context

P3.2 added gateway-central rate limiting (`governor` token bucket, in-memory). When business logic needs finer-grained limits — e.g. "max 5 announcements per admin per minute" — gateway can't express that. It only knows routes + identity.

This adds **service-side rate limiting** as an opt-in middleware:
- Notification-service gets it first (anti-spam use case from roadmap)
- Token-service / iam-service can opt-in later for similar needs
- Library `common-ratelimit` so all services share the implementation

## Decision

### Algorithm: Sliding Window via Redis ZSET

**Why not `governor` again?** `governor` is in-memory, which means each service replica has its own bucket. With 2 replicas, a user gets 2x the limit. Acceptable for gateway (single instance today), but service-side limits need **shared state**.

**Redis sorted set per (route, identity)**:
- Key: `rl:<route>:<identity>:<window_seconds>`
- Score: timestamp ms
- Value: timestamp ms (ZSET member)
- On each request: ZADD timestamp, ZREMRANGEBYSCORE old entries, ZCARD count
- If count > limit, reject with 429

Trade-off:
- ✅ Shared across replicas
- ✅ Sliding window (more accurate than fixed window)
- ✅ Per-route, per-identity granularity
- ❌ ~5 Redis ops per request (vs in-memory governor)
- ❌ Adds Redis dependency to services that previously didn't need it

### For notification-service

Limits to enforce:
- `POST /announcements`: 5 per admin per minute (anti-spam)
- `POST /announcements/{id}/publish`: 10 per admin per minute
- `POST /announcements/{id}/revoke`: 10 per admin per minute
- All other routes: 60 per user per minute (generic)

### Library: `libs/common-ratelimit`

```rust
pub struct RateLimiter { /* Redis client, default config */ }

pub struct Limit { pub route: &'static str, pub max: u32, pub window_secs: u32 }

impl RateLimiter {
    pub async fn check(&self, identity: &str, limit: &Limit) -> Result<(), RateLimitExceeded>;
}
```

Middleware factory `axum_middleware_from_fn_with_state` to wire into Router.

### Response format (matches gateway)

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 1
Content-Type: application/json

{
  "code": "RATE_LIMITED",
  "message": "too many requests",
  "retry_after_ms": 1000
}
```

### Identity extraction

For notification-service, Identity comes from existing `auth::Identity` extractor (X-User-Id / JWT). Falls back to peer IP if anonymous.

### Observability

Prometheus counter `service_rate_limited_total{route="...", service="notification-service"}`. Per-service because the metric is generic — Grafana shows per-service.

### Backward compatibility

- **Existing services unchanged.** Notification-service is the first opt-in.
- **Gateway still does its own rate limit** (P3.2). The two layers compose: gateway checks route-level limits, service checks business-logic limits (e.g. "5 announcements per admin per minute").
- **Failure mode**: if Redis is down, service-side rate limiter **fails open** (allows request) rather than fails closed (rejects). Log a warning. Reason: Redis going down shouldn't break the whole product.

## Files Touched

| File | Change |
|------|--------|
| `libs/common-ratelimit/Cargo.toml` | NEW — depends on `governor` + `redis` + `axum` + `tokio` |
| `libs/common-ratelimit/src/lib.rs` | NEW — `RateLimiter`, `Limit`, axum middleware factory |
| `services/notification-service/Cargo.toml` | Add common-ratelimit |
| `services/notification-service/src/rest.rs` | Apply rate-limit middleware to admin routes |
| `services/notification-service/src/lib.rs` | Export new module |
| `services/notification-service/src/auth.rs` | Add identity extraction helper if missing |
| `services/notification-service/tests/rate_limit_test.rs` | NEW — unit tests |
| `services/notification-service/tests/rate_limit_integration_test.rs` | NEW — Redis-backed integration test |
| `infrastructure/config/api-gateway/rate-limits.toml` | Adjust notification limits in gateway (since service now also enforces) |
| `docs/superpowers/specs/2026-06-25-service-ratelimit-design.md` | NEW (this doc) |
| `docs-site/USER_MANUAL.md` § 7 | Update with dual-layer concept |
| `docs-site/guides/observability.md` | Add `service_rate_limited_total` panel |

## Verification

1. Unit tests: rate limiter math + limits table
2. Integration: real Redis with sliding window verification
3. E2E: e2e/admin hammer test for 5/min limit
4. CI: `node scripts/check-service-standard.js` exit 0
5. Manual: hit `POST /announcements` 6 times in 30s → first 5 succeed, 6th gets 429

## Out of Scope

- Cross-service shared rate limit (e.g. "10 announcements across all services") — each service has its own bucket
- Distributed token bucket algorithm (e.g. GCRA) — sliding window is enough for this use case
- Per-IP rate limiting at service level — gateway handles it