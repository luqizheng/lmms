# P3.4 Canary Release via Nginx + Consul Health Check

## Decision

**Adopt recommended path: Nginx upstream weight + Consul TTL health check.**

## Context

No canary mechanism today. Rollouts are all-at-once via `docker compose up -d`. With 8 services and growing traffic, need:
- Gradual rollout (10% → 50% → 100%)
- Fast rollback (weight flip)
- Automatic unhealthy instance removal

## Implementation

### 1. Tag-based canary via Consul service tags

Services register themselves in Consul with tags. A new tag `canary` marks a version as canary:

```json
{
  "Name": "audit-service",
  "Tags": ["v1.2.0", "canary"],   // ← canary version
  "Address": "...",
  "Port": 8080
}
```

Standard instances omit the `canary` tag.

### 2. Nginx upstream + Consul template

Two options considered:

**A. consul-template + reload nginx** (chosen)
- `consul-template` watches Consul for `canary` tag presence
- Renders nginx config from template
- Reloads nginx on change
- Battle-tested, simple

**B. nginx + nginx-upsync-module**
- Dynamic upstream without reload
- More complex, requires custom nginx build

Choose **A** for low-memory platform alignment.

### 3. Routing rules

```nginx
# Default: 100% stable
upstream audit-service-stable {
  server audit-service-stable:8080;
}

# Canary: opt-in via header or % traffic
upstream audit-service-canary {
  server audit-service-canary:8080;
}

# Split by weight
split_clients $request_id $audit_backend {
  10%   audit-service-canary;
  *     audit-service-stable;
}

location /api/v1/audit/ {
  proxy_pass http://$audit_backend;
}
```

Plus a `?canary=1` query param / `X-Canary: true` header to force canary for testing.

### 4. Health check integration

- Consul TTL: each service re-registers every 30s via `/consul/keepalive`
- Failure → Consul marks instance `critical` → consul-template regenerates nginx without it
- Nginx `health_check` directive on upstream block (active probes)

### 5. Rollback procedure

```bash
# Rollback canary to 0%
docker compose -f infrastructure/docker-compose-canary.yml down audit-service-canary
# OR
docker exec consul-container consul tags -delete canary audit-service-canary-id
```

Both ways: within 10 seconds (consul-template refresh interval), traffic returns to 100% stable.

## Files Touched

| File | Change |
|------|--------|
| `infrastructure/config/nginx/nginx.conf.template` | NEW Consul template |
| `infrastructure/config/nginx/consul-template.hcl` | NEW |
| `infrastructure/docker-compose-observability.yml` | Add consul-template sidecar |
| `infrastructure/docker-compose.yml` | Document `canary` tag convention |
| `services/*/src/main.rs` | Add `consul.register(tags: vec!["v1.0.0"])` helper |
| `services/api-gateway/src/discovery.rs` | Accept tag filter param |
| `docs/guides/canary-rollout.md` | NEW runbook |
| `docs/superpowers/specs/2026-06-25-canary-design.md` | NEW (this doc) |

## Verification

1. Deploy `audit-service-canary` with `canary` tag → nginx upstream shows 1 instance
2. Curl 100 times → ~10 land on canary (per `split_clients` 10%)
3. Force `X-Canary: true` header → 100% land on canary
4. Stop canary container → nginx upstream removes it within 10s
5. Document procedure in `docs/guides/canary-rollout.md`

## Out of Scope

- A/B testing with rule-based splits (mesh solution needed)
- Multi-region failover (single region only)
- Sticky sessions (currently stateless)