# Canary Rollout Runbook

Step-by-step guide for rolling out a new version of any service via canary (10% → 50% → 100%).

## When to use

- Production rollout where you want gradual traffic shift
- New version might break but you want to validate with real traffic
- You need fast rollback capability

## Architecture

- **Nginx** fronts all 8 services via Consul service discovery
- **consul-template** watches Consul every 2–10s and re-renders nginx config
- A service instance tagged `canary` in Consul receives 10% of traffic initially
- `?canary=1` query param or `X-Canary: true` header forces canary for testing

## Step 1: Deploy canary instance

Add the new version with a different service name suffix or use a tag.

```bash
# Option A: separate canary stack via compose override
docker compose -f infrastructure/docker-compose-canary.yml up -d audit-service-canary

# Option B: tag the existing instance (not recommended for production)
# but useful for quick testing:
consul-cli tag add -service audit-service -tag canary -id <instance-id>
```

The `audit-service-canary` instance must register itself in Consul with tag `canary`.

## Step 2: Verify Consul sees the canary

```bash
curl http://localhost:8500/v1/health/service/audit-service?tag=canary
```

Should return the canary instance(s).

## Step 3: Watch nginx config update

```bash
docker exec observability-consul-template consul-template -config=/etc/consul-template/config.hcl \
  -dry -once 2>&1 | grep -A 5 "audit-service"
```

You should see both `-stable` and `-canary` upstreams.

## Step 4: Smoke-test canary directly

```bash
# Force canary via header
curl -H "X-Canary: true" https://api.example.com/v1/audit/health
# Should hit canary instance

# Without header → ~10% should hit canary over many requests
for i in {1..100}; do curl -s -o /dev/null -w "%{http_code}\n" https://api.example.com/v1/audit/health; done
```

## Step 5: Monitor metrics

Open Grafana → Gateway Overview dashboard:
- Request rate per service (split into stable vs canary via `instance` label)
- Error rate should remain flat
- Latency p95 should match stable

If error rate spikes on canary:
```bash
# Roll back canary
docker compose -f infrastructure/docker-compose-canary.yml down audit-service-canary
# OR
consul-cli tag delete -service audit-service -tag canary -id <instance-id>
```

Within 10 seconds, all traffic returns to stable.

## Step 6: Promote to 100%

Once confident:

```bash
# Tear down stable stack (after canary succeeds)
docker compose -f infrastructure/docker-compose.yml up -d --scale audit-service-canary=2

# OR: redeploy stable with new image
docker compose -f infrastructure/docker-compose.yml up -d audit-service
docker compose -f infrastructure/docker-compose-canary.yml down
```

The canary tag is removed; all instances become stable.

## Adjusting traffic split

Edit `infrastructure/config/nginx/nginx.conf.template`:

```nginx
split_clients $request_id $audit-service_backend {
    50%   audit-service-canary;  # was 10%
    *     audit-service-stable;
}
```

consul-template reloads nginx automatically within ~10s.

## Rollback checklist

- [ ] Identify the issue from logs/metrics
- [ ] `docker compose -f infrastructure/docker-compose-canary.yml down <svc>-canary`
- [ ] Verify `gateway_request_rate{instance=...canary}` returns to 0 in Grafana
- [ ] Postmortem within 24h, file issue/PR

## Out of scope

- A/B testing with rule-based splits (use mesh solution)
- Multi-region failover
- Sticky sessions (stateless services only)