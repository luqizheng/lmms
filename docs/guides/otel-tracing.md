# OpenTelemetry Traces Runbook

How to enable, observe, and debug distributed tracing with the OTel stack.

## Architecture

```
┌─────────────┐  OTLP gRPC :4317   ┌──────────────────┐
│ api-gateway │ ─────────────────► │ otel-collector   │
└─────────────┘                    │  (batching /     │
                                   │   enrichment)    │
                                   └────────┬─────────┘
                                            │
                          ┌─────────────────┼─────────────────┐
                          ▼                                   ▼
                  ┌───────────────┐                  ┌────────────────┐
                  │ Jaeger :16686 │                  │ Prometheus     │
                  │   (UI)        │                  │   :8889 (RED   │
                  │   traces UI   │                  │    metrics)    │
                  └───────────────┘                  └────────────────┘
```

## Step 1: Bring up the trace stack

```bash
docker compose -f infrastructure/docker-compose.yml up -d
scripts/start-observability.sh
```

This adds:
- `otel-collector` (OTLP receiver on :4317 gRPC / :4318 HTTP)
- `jaeger` (UI on http://localhost:16686)

## Step 2: Enable OTel in a service

The `common-telemetry` crate has the OTel layer gated behind the `otlp` feature. It activates only when `OTEL_EXPORTER_OTLP_ENDPOINT` env is set.

For `api-gateway` (already wired in this repo):
```bash
# Add to infrastructure/docker-compose.yml under api-gateway:
environment:
  OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
```

For other services, opt in by:
1. Adding the dep with `features = ["otlp"]`:
   ```toml
   common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }
   ```
2. Calling `common_telemetry::init_tracing("service-name")` at startup.
3. Setting `OTEL_EXPORTER_OTLP_ENDPOINT` env.

Without the env var, services fall back to JSON stdout (zero behavior change).

## Step 3: Verify a trace appears

1. Send a request through gateway:
   ```bash
   curl -i http://localhost:3000/v1/iam/health
   ```
   Look at response headers — there should be `X-Trace-Id: <32-hex>`

2. Open Jaeger UI: http://localhost:16686
   - Service: `api-gateway`
   - Operation: `http_request`
   - Click "Find Traces"

3. You should see a span tree:
   ```
   http_request
     ├─ auth_middleware
     ├─ rate_limit_middleware
     └─ proxy_handler
         └─ (downstream HTTP call — trace_id chains)
   ```

## Step 4: Correlate logs ↔ traces

Every log line emitted inside the request span carries `trace_id` (auto-injected by `tracing-opentelemetry`). Filter logs:
```bash
docker logs api-gateway 2>&1 | grep "<trace_id>"
```

## Step 5: Cross-service trace chains

When api-gateway proxies to a downstream service, it injects the W3C `traceparent` header into the outgoing request. Downstream services that also run with OTel enabled will pick it up and continue the trace.

```yaml
# A trace from gateway → token-service → iam-service:
api-gateway         POST /v1/auth/login
  └─ http_request
       └─ proxy_handler
            └─ token-service  POST /token/issue
                 └─ http_request
                      └─ iam-service  GET /iam/users/{sub}
                           └─ http_request
```

Look up the trace in Jaeger by `service.api-gateway` and you'll see the entire tree.

## Common issues

### "I enabled OTEL_EXPORTER_OTLP_ENDPOINT but no traces appear in Jaeger"

1. Check the service can reach the collector:
   ```bash
   docker exec api-gateway curl -v http://otel-collector:4317
   ```
2. Check collector logs:
   ```bash
   docker logs observability-otel-collector | tail -50
   ```
3. Check Jaeger received anything:
   ```bash
   curl http://localhost:16686/api/services
   # Should list api-gateway after a request was made
   ```

### "Service starts but traces have no service.name"

The `service_name` parameter to `init_tracing` is what OTel attaches as the `service.name` resource attribute. Check your init call passes a non-empty string.

### "High CPU / memory usage"

OTel batches spans. Default batch size is 1000 or 5s timeout, whichever comes first. Override:
```bash
OTEL_BSP_SCHEDULE_DELAY=10000  # batch every 10s
OTEL_BSP_MAX_BATCH_SIZE=500
```

### "Want to disable without redeploy"

Don't set `OTEL_EXPORTER_OTLP_ENDPOINT`. The service falls back to stdout-only tracing automatically.

## Sampling

Default: 100% (every request traced). To sample 10%:
```bash
OTEL_TRACES_SAMPLER=TraceIdRatio
OTEL_TRACES_SAMPLER_ARG=0.1
```

For head-based sampling at the gateway: configure in collector.yaml's `tail_sampling` processor.

## Out of scope

- Metrics from spans (RED metrics): emitted by OTel collector's prometheus exporter, available at :8889
- Logs correlation: requires `tracing-subscriber` json formatter (already enabled by default)
- Custom span attributes on business events (e.g. `menu_config.etag_conflict`): add `#[instrument]` or manual `tracing::info_span!` to specific code paths