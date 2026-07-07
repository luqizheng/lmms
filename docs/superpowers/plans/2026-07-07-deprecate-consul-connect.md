# De-phase Consul Connect — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drop the entire Consul Connect stack (sidecars / intentions / consul-dataplane / connect-proxy), deploy a single `gliderlabs/registrator` container as the project-level service registry, and refactor 7 Rust services to call each other over plain HTTP with `Authorization: Bearer <jwt>` passthrough.

**Architecture:** Registrator watches `/var/run/docker.sock`, reads `SERVICE_*` labels on each business container, and auto-registers / deregisters them in Consul's catalog via the agent HTTP API. Consul keeps its DNS / catalog / K/V / health-check roles but no longer terminates any service traffic. Service-to-service authn shifts from mTLS to JWT passthrough (`/iam/internal/*` already accepts `Audience::Any`). Memory drops by ~400MB (6 sidecars replaced by one registrator).

**Tech Stack:** Rust 1.96 (workspace builds with `cargo`), Docker Compose v2 (`docker compose`), Consul 1.15 (dev `-dev` mode), `gliderlabs/registrator:master` (later pinned to a digest), Axum 0.8 HTTP client, `reqwest 0.12 + rustls` (TLS disabled at dev).

**Spec:** `docs/superpowers/specs/2026-07-07-deprecate-consul-connect-use-registrator-design.md` (must be committed before this plan starts).

---

## Global Constraints

- **Working tree must be clean before Task 1.** Today it carries the sidecar-rollback diff (image line of all 6 sidecars back to `consul:1.15`). Task 3 supersedes that diff in a single atomic commit, so do **not** commit the diff before Task 3. If anything else accumulates, `git stash` and apply at Task 3.
- **Compose spec is canonical.** `docker compose -f infrastructure/docker-compose.yml config` output drives all verification — never read the YAML and guess.
- **`consul` CLI on the consul container is the inspection surface.** All Consul catalog queries run via `docker exec infrastructure-consul-1 consul catalog ...`. The host's `consul` CLI is not installed.
- **No new DB tables.** Schema is fixed.
- **No new external crates added to `Cargo.toml`.** The plan only deletes `libs/consul-registrar` and pulls its calls out of service mains.
- **`node scripts/run-tests-docker.js <svc>`** is the integration-test runner for Rust services. Each task that touches a service main runs the relevant subset as a gate.
- **`scripts/` shell scripts use `set -euo pipefail`.** Idempotent where possible (registrator relies on Docker events; intentions.sh is removed so idempotency is moot for that one).
- **Commit prefix per task is `dephase:`** (e.g. `dephase(infra): add registrator, drop consul-init`, `dephase(token): jwt passthrough`).
- **No `agent  -dev` changes beyond what's listed.** Consul still runs `-dev` mode; we only disable `connect { enabled = true }`.
- **Per AGENTS.md "一个任务一个 commit"**. Each task ends in exactly one commit.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `infrastructure/docker-compose-infra.yml` | modify | Add `registrator`; remove `consul-init`; pass docker.sock volume |
| `infrastructure/config/consul/connect.hcl` | modify | Strip `connect { enabled = true }` block |
| `infrastructure/.env` | modify | Add `SERVICE_*` env vars consumed by labels anchor |
| `infrastructure/docker-compose-services.yml` | modify | Add `x-svc-registrator-labels` anchor; drop 7 sidecars; flip `SERVER_PORT` to 808x; drop `network_mode` |
| `infrastructure/config/api-gateway/api-gateway.toml` | modify | Upstream URLs back to true ports (`iam-service:8081` etc) |
| `services/api-gateway/src/main.rs` | modify | Remove `ConsulRegistrar::new` block; remove sidecar-specific env wiring |
| `services/api-gateway/src/router.rs` | modify | Drop `egress_map` if present (was mesh-only); use plain upstream URLs |
| `services/iam-service/src/main.rs` | modify | Remove `ConsulRegistrar::new`; allow `Audience::Any` on `/iam/internal/*`; flip SERVER_PORT to 8081 |
| `services/iam-service/src/config.rs` | modify | Default `SERVER_PORT=8081`, `SERVER_HOST=0.0.0.0` |
| `services/token-service/src/main.rs` | modify | Remove `ConsulRegistrar::new`; flip SERVER_PORT to 8082 |
| `services/token-service/src/issuer.rs` | modify | Internal fetch to iam: pass `Authorization: Bearer <user-jwt>` instead of sidecar mTLS |
| `services/token-service/src/config.rs` | modify | Default `SERVER_PORT=8082` |
| `services/notification-service/src/main.rs` | modify | Remove `ConsulRegistrar::new`; internal users lookup passes Authorization; flip SERVER_PORT to 8083 |
| `services/notification-service/src/config.rs` | modify | Default `SERVER_PORT=8083` |
| `services/audit-service/src/main.rs` | modify | Remove `ConsulRegistrar::new`; flip SERVER_PORT to 8084 |
| `services/audit-service/src/config.rs` | modify | Default `SERVER_PORT=8084` |
| `services/config-service/src/main.rs` | modify | Remove `ConsulRegistrar::new`; flip SERVER_PORT to 8081 |
| `services/config-service/src/config.rs` | modify | Default `SERVER_PORT=8081` |
| `services/file-service/src/main.rs` | modify | Remove `ConsulRegistrar::new`; flip SERVER_PORT to 8082 |
| `services/file-service/src/config.rs` | modify | Default `SERVER_PORT=8082` |
| `services/prue_backend/src/main.rs` | modify | Remove `ConsulRegistrar::new`; flip SERVER_PORT to 8087 |
| `services/prue_backend/src/config.rs` | modify | Default `SERVER_PORT=8087` |
| `Cargo.toml` (workspace) | modify | Drop `members = [..., "libs/consul-registrar"]` line |
| `libs/consul-registrar/Cargo.toml` | delete | — |
| `libs/consul-registrar/src/lib.rs` | delete | — |
| `libs/consul-registrar/tests/*` | delete | — |
| `services/<each>/Cargo.toml` (7 services) | modify | Remove `consul-registrar = { workspace = true }` dependency |
| `scripts/consul-intentions.sh` | delete | No longer needed |
| `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md` | modify | Add "Superseded by 2026-07-07-..." banner |
| `docs/superpowers/plans/2026-06-29-phase5a-mtls-sidecar.md` | modify | Add "Superseded" banner |
| `docs/ai-context.md` | modify | Update service map and "不要做" list |
| `docs/guides/mesh-service-onboarding.md` | delete or repurpose | Replace with `service-to-service-auth.md` |
| `docker-example/services/*/Dockerfile` (if listed) | modify | Drop sidecar assumptions, document direct-URL + JWT usage |
| `docker-example/docker-compose*.yml` | modify | Mirror the production compose changes |

No new files outside this list unless a task's Step explicitly says to create one.

---

## Task 1: Add `registrator`; disable Consul Connect; drop `consul-init`

**Files:**
- Modify: `infrastructure/docker-compose-infra.yml`
- Modify: `infrastructure/config/consul/connect.hcl`

**Interfaces:**
- Consumes: nothing (clean working tree prior, but uncommitted sidecar-rollback diff stays — Task 3 absorbs it)
- Produces: a `registrator` service on the `superpowers` network, mounting `/var/run/docker.sock`; a Consul config with `connect { enabled = true }` removed; no `consul-init` container; the existing token/`iam`/`audit`/`prue`/`notification`/`api-gateway` sidecars still present and still broken in their own right (Task 3 fixes them)

- [ ] **Step 1: Confirm working tree is dirty with the sidecar-rollback diff; do not stage it**

Run:
```bash
git status --short -- infrastructure/
```
Expected: a `M` next to `infrastructure/docker-compose-services.yml` and possibly `scripts/consul-intentions.sh` (chmod only). These changes are intentionally deferred to Task 3's atomic commit.

- [ ] **Step 2: Strip `connect { enabled = true }` from Consul config**

Edit `infrastructure/config/consul/connect.hcl`. The full target contents:

```hcl
datacenter = "dc1"
data_dir = "/consul/data"

ports {
  grpc = 8502
}
```

That is — delete the entire `connect { enabled = true }` block. Run:

```bash
cat infrastructure/config/consul/connect.hcl
```
Expected:
```
datacenter = "dc1"
data_dir = "/consul/data"

ports {
  grpc = 8502
}
```

- [ ] **Step 3: Add `registrator` to compose; remove `consul-init`**

Edit `infrastructure/docker-compose-infra.yml`. In the `services:` block, **delete** the entire `consul-init:` service definition (it currently looks like):

```yaml
  consul-init:
    image: hashicorp/consul:2.0.1
    depends_on:
      consul:
        condition: service_healthy
    volumes:
      - ../scripts/consul-intentions.sh:/consul-intentions.sh:ro
    command: sh -c "sh /consul-intentions.sh && echo 'consul-init complete'"
    network_mode: "service:consul"
    restart: "no"
    profiles:
      - services
```

After that block, **add** the `registrator` service at the end of the `services:` block:

```yaml
  registrator:
    image: gliderlabs/registrator:master
    depends_on:
      consul:
        condition: service_healthy
    volumes:
      - /var/run/docker.sock:/tmp/docker.sock
    environment:
      DOCKER_HOST: unix:///tmp/docker.sock
      REGISTRATOR_RESYNC: "30s"
    command: ["consul://consul:8500"]
    restart: unless-stopped
    networks:
      - superpowers
```

Save the file.

- [ ] **Step 4: Render and verify**

Run:
```bash
docker compose -f infrastructure/docker-compose.yml config | grep -E "registrator:|consul-init:|connect " | head
```
Expected output (must contain `registrator:` and **must not** contain `consul-init:` or `connect:` at any depth):

```
      registrator:
```

If `consul-init:` still appears, the deletion in Step 3 didn't take — go back.

- [ ] **Step 5: Restart Consul only, then bring up registrator**

Run:
```bash
docker compose -f infrastructure/docker-compose.yml up -d --force-recreate consul registrator
sleep 6
docker compose -f infrastructure/docker-compose.yml ps consul registrator
```
Expected: both `Up (healthy)` or `Up`. Network errors at this stage are normal; we just want them running.

- [ ] **Step 6: Smoke-test registrator against Consul**

Run:
```bash
docker exec infrastructure-consul-1 consul catalog services
docker logs infrastructure-registrator-1 --tail 5
```
Expected:
- `consul catalog services` returns `consul` (just the Consul-internal service, no business services yet — labels are not in place until Task 2)
- Registrator logs include a line like `connecting to backend` -> `consul://consul:8500`. No panics.

- [ ] **Step 7: Commit**

Run:
```bash
git add infrastructure/docker-compose-infra.yml infrastructure/config/consul/connect.hcl
git commit -m "dephase(infra): add registrator, drop consul-init and connect enabled"
```

---

## Task 2: Wire `SERVICE_*` labels on all 7 business services

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`
- Modify: `infrastructure/.env`

**Interfaces:**
- Consumes: spec §2.2 (port allocation), spec §3.2 (label template)
- Produces: a YAML anchor `x-svc-registrator-labels` and per-service `labels:` blocks; an `.env` with `SERVICE_*` variables per service

- [ ] **Step 1: Add labels anchor to compose**

Open `infrastructure/docker-compose-services.yml`. At the top of the file, **before** the `services:` key, add:

```yaml
x-svc-registrator-labels: &svc-reg-labels
  - "SERVICE_NAME=${SERVICE_NAME}"
  - "SERVICE_ID=${SERVICE_ID}"
  - "SERVICE_PORT=${SERVICE_PORT}"
  - "SERVICE_TAGS=backend,${SERVICE_NAME}"
  - "SERVICE_${SERVICE_PORT}_CHECK_HTTP=${SERVICE_HEALTH_PATH}"
  - "SERVICE_${SERVICE_PORT}_CHECK_INTERVAL=15s"
  - "SERVICE_${SERVICE_PORT}_CHECK_TIMEOUT=3s"

services:
```

That is — the entire existing `services:` block stays; the anchor goes ABOVE it.

- [ ] **Step 2: Append per-service blocks**

For each of the 7 services — `api-gateway`, `iam-service`, `token-service`, `notification-service`, `audit-service`, `config-service`, `file-service`, `prue_backend` (note `prue_backend` is the 8th; api-gateway included) — add a `labels:` mapping and a `environment:` block (or extend existing environment) keyed to that service.

The exact values per service:

| Service | SERVICE_NAME | SERVICE_ID | SERVICE_PORT | SERVICE_HEALTH_PATH |
|---|---|---|---|---|
| `api-gateway` | `api-gateway` | `api-gateway` | `3000` | `/health` |
| `iam-service` | `iam-service` | `iam-service` | `8081` | `/health` |
| `token-service` | `token-service` | `token-service` | `8082` | `/health` |
| `notification-service` | `notification-service` | `notification-service` | `8083` | `/health` |
| `audit-service` | `audit-service` | `audit-service` | `8084` | `/health` |
| `config-service` | `config-service` | `config-service` | `8081` | `/health` |
| `file-service` | `file-service` | `file-service` | `8082` | `/health` |
| `prue_backend` | `prue_backend` | `prue_backend` | `8087` | `/health` |

For each service, the patch is:

```yaml
  <svc-name>:
    ...
    labels: *svc-reg-labels
    environment:
      ...existing env...
      SERVICE_NAME: "<svc>"
      SERVICE_ID: "<svc>"
      SERVICE_PORT: "<port>"
      SERVICE_HEALTH_PATH: "/health"
```

**Important**: `iam-service` and `config-service` both register on port `8081`; they are different containers on the same Docker network — no host port conflict yet (host-port mapping unchanged for now; Task 3 handles `config-service: 8085:8081`). `token-service` and `file-service` both register on port `8082` (container-internal); Task 3 handles host mapping.

> ⚠️ `file-service` is currently bound to `8082:8082` (host:container). This host collision with `token-service` is resolved in Task 3 by moving `file-service` host port to `8086`. Registrator labels only care about container-internal port, so no labels-level adjustment here.

- [ ] **Step 3: Annotate `.env`**

Append the following block to `infrastructure/.env` (placed after the existing entries — this is a hint, no semantic ordering required):

```bash
# registrator: service discovery (Consul) via gliderlabs/registrator
# Per-service labels filled in by docker-compose via ${SERVICE_*}
# Default values to remove any label warnings; the compose file overrides
# SERVICE_NAME=
# SERVICE_ID=
# SERVICE_PORT=
# SERVICE_HEALTH_PATH=
```

The compose-time `${SERVICE_*}` references are absolute-required even when blank; if compose warns "variable not set", it means a per-service override (Step 2) is missing for that service.

- [ ] **Step 4: Render and grep service labels**

Run:
```bash
docker compose -f infrastructure/docker-compose.yml config | grep -E "SERVICE_(NAME|ID|PORT|TAGS|.*CHECK)" | head -40
```
Expected: at least 30 lines (8 services × ~7 keys; precise count varies) — every value should be a literal (no `${...}` left unresolved). If any `${...}` survives, one of the services in Step 2 is missing its `environment:` block.

- [ ] **Step 5: Bring up all 7 business services (no sidecar drops yet)**

```bash
docker compose -f infrastructure/docker-compose.yml up -d --force-recreate --no-deps \
  api-gateway iam-service token-service notification-service \
  audit-service config-service file-service prue_backend
```
(This will not work fully yet — `network_mode: service:<svc>-sidecar` from the old sidecar setup is still in place, so containers will fail to start. That's expected. The point of this step is only to **prove labels render correctly.** If you want a quick sanity check without trying to start all of them, restart just `iam-service` after temporarily commenting out its `network_mode:`, then revert.)

Acceptable alternative: skip Step 5 and proceed to Step 6. The labels have rendered; the actual registration test happens after Task 3.

- [ ] **Step 6: Commit**

```bash
git add infrastructure/docker-compose-services.yml infrastructure/.env
git commit -m "dephase(infra): add SERVICE_* labels anchor and per-service env"
```

---

## Task 3: Atomic cutover — drop sidecars, flip ports, update gateway upstream

**Files:**
- Modify: `infrastructure/docker-compose-services.yml` (large edit)
- Modify: `infrastructure/config/api-gateway/api-gateway.toml` (or wherever the upstream map lives; `services/api-gateway/src/main.rs` if it's hardcoded)
- Modify: `services/<each>/src/config.rs` (7 services) — default `SERVER_PORT`
- Modify: `services/<each>/src/main.rs` (7 services) — drop `network_mode: service:X-sidecar`, drop old-sidecar env, flip `SERVER_HOST`

**Interfaces:**
- Consumes: Task 1 (registrator running); Task 2 (labels in place); the dirty working tree from earlier session (rolled into this commit)
- Produces: 7 services connect directly on `808x`; no sidecar containers; gateway upstream URLs use true ports

This is the central cutover. The unit is one commit; rolling back this commit reverts the entire mesh-removal state. **Take care.**

- [ ] **Step 1: Confirm pre-state**

```bash
git status --short
```
Expected: clean (Tasks 1 & 2 are committed). If you still see the old sidecar-rollback diff (`M infrastructure/docker-compose-services.yml`), that's fine — it's your cue to fold the right answers into the same commit.

- [ ] **Step 2: Delete the 7 sidecar services**

In `infrastructure/docker-compose-services.yml`, delete the following blocks entirely (anywhere they appear):

- `api-gateway-sidecar`
- `token-service-sidecar`
- `iam-service-sidecar`
- `prue_backend-sidecar`
- `notification-service-sidecar`
- `audit-service-sidecar`

Six services total — `config-service` and `file-service` never had sidecars; they don't appear here.

- [ ] **Step 3: Drop `network_mode: "service:<svc>-sidecar"` from each business service**

For each of the 6 services that previously had a sidecar, delete the line:

```yaml
    network_mode: "service:<svc>-sidecar"
```

Now each business service owns its own network namespace and binds its `ports:` directly to the `superpowers` bridge.

- [ ] **Step 4: Update host port mappings — resolve conflicts**

Per spec §7:

| Service | Old host-port | New host-port | Container port |
|---|---|---|---|
| `config-service` | `8081:8081` | `8085:8081` | `8081` |
| `file-service` | `8082:8082` | `8086:8082` | `8082` |
| Others | unchanged | unchanged | unchanged |

Edit the `ports:` block of each affected service. Example for `config-service`:

```yaml
    ports:
      - "8085:8081"
```

- [ ] **Step 5: Flip `SERVER_PORT` defaults in each `config.rs`**

Open `services/<name>/src/config.rs` (or `.yaml` for notification) and change the default port to:

| Service | Default SERVER_PORT | SERVER_HOST |
|---|---|---|
| `api-gateway` | `3000` (unchanged) | `0.0.0.0` (unchanged) |
| `iam-service` | `8081` | `0.0.0.0` |
| `token-service` | `8082` | `0.0.0.0` |
| `notification-service` | `8083` | `0.0.0.0` |
| `audit-service` | `8084` | `0.0.0.0` |
| `config-service` | `8081` (container-internal; host mapping `8085:8081`) | `0.0.0.0` |
| `file-service` | `8082` (container-internal; host mapping `8086:8082`) | `0.0.0.0` |
| `prue_backend` | `8087` | `0.0.0.0` |

Use existing config-loader conventions (`Loader::new("<NAME>").separator("_")` style). For each, find the existing `8081` / `18081` / `18082` default and replace.

Note: services previously running on `180xx` had their `SERVER_PORT` env overridden by compose to `18081` etc. We're deleting those compose overrides too. After this step, defaults of 808x compose with no overrides.

- [ ] **Step 6: Delete compose env overrides that target ports / mTLS / consul-dataplane**

In each business service's `environment:` block, **delete** any of these keys (they are mesh-only):

```yaml
        SERVER_HOST: "127.0.0.1"
        SERVER_PORT: "18081"
        NETWORK_MODE_BIND: "..."
        CONSUL_HTTP_ADDR: "..."
        DP_CONSUL_ADDRESSES: "..."
        DP_CONSUL_GRPC_PORT: "..."
        DP_SERVICE_NODE_NAME: "..."
        DP_PROXY_SERVICE_ID: "..."
        DP_TLS_DISABLED: "..."
        DP_LOG_LEVEL: "..."
```

Their presence in the compose file today is the symptom of mixed state. Leaving them in is a footgun, since the future operator reading the file will wonder.

- [ ] **Step 7: Update api-gateway upstream URLs**

Locate `infrastructure/config/api-gateway/api-gateway.toml` (or wherever upstream URLs are defined). Replace any upstream URL currently pointing at sidecar ports (9081 / 9082 / 9083 / 9084 / 9085 / 9086 / 9087) with the true-service port:

| Service | Old upstream | New upstream |
|---|---|---|
| iam-service | `iam-service:9081` | `iam-service:8081` |
| token-service | `token-service:9082` | `token-service:8082` |
| notification-service | `notification-service:9083` | `notification-service:8083` |
| audit-service | `audit-service:9084` | `audit-service:8084` |
| config-service | `config-service:8081` | `config-service:8081` (unchanged — was already direct) |
| file-service | `file-service:8082` | `file-service:8082` (unchanged — was already direct) |
| prue_backend | `prue_backend:9087` | `prue_backend:8087` |

Also remove the `egress_map` / mesh-specific keys if they exist.

If upstream URLs are built in code (`services/api-gateway/src/router.rs` or similar) instead of TOML, edit the source accordingly: any reference to a hard-coded sidecar port in the form `9081`/`9082`/etc gets the matching `8081`/`8082`/etc.

- [ ] **Step 8: Render sanity**

Run:
```bash
docker compose -f infrastructure/docker-compose.yml config | grep -E "^-? ?\"?[0-9]+:[0-9]+\"?" | grep -v "^#" | sort -u | head -30
```
Expected:
- Container-internal `8081` / `8082` / `8083` / `8084` / `8087` / `3000` appear in services' `ports:`
- `8085:8081` and `8086:8082` appear (host mappings for config / file)
- No `908x` survives

If `9081` / `9082` / etc. still appear anywhere, you missed a step — find and fix before continuing.

- [ ] **Step 9: Smoke-test the full stack**

```bash
docker compose -f infrastructure/docker-compose.yml down
docker compose -f infrastructure/docker-compose.yml up -d
sleep 30
docker compose -f infrastructure/docker-compose.yml ps
```
Expected:
- All 8 business services `Up (healthy)`
- `frontend` `Up`
- `consul`, `mysql`, `redis`, `rabbitmq`, `registrator` all `Up (healthy)`

- [ ] **Step 10: Verify registrator + Consul catalog**

```bash
docker exec infrastructure-consul-1 consul catalog services
docker exec infrastructure-consul-1 consul catalog nodes
```
Expected services (8):
```
api-gateway
audit-service
config-service
consul
file-service
iam-service
notification-service
prue_backend
token-service
```
(Plus `consul` — that's Consul's own service.)

Expected nodes: only the Consul server node (no `token-service-sidecar` etc.). The service names appear under the `consul-server` node's catalog.

If fewer than 9 services appear, registrator did not pick them up — check labels via `docker inspect <service>` and look at registrator's logs (`docker logs infrastructure-registrator-1`).

- [ ] **Step 11: Smoke-test gateway → iam**

```bash
docker exec infrastructure-consul-1 curl -sf http://iam-service:8081/health | head -3
docker exec infrastructure-consul-1 curl -sf http://api-gateway:3000/health | head -3
```
Expected: HTTP 200 with a JSON body (or whatever the existing `/health` returns). If `curl` returns connection refused, the upstream port fix is incomplete.

- [ ] **Step 12: Commit (atomic cutover)**

```bash
git add -A
git status --short
git commit -m "dephase(infra): drop mesh, services talk direct via docker DNS to 808x, registrator owns catalog"
```

> This commit absorbs the earlier session's sidecar-rollback diff. The pre-existing dirty state disappears here. Verify `git log -1 --stat` shows the full expected change-set.

---

## Task 4: Authorization passthrough — `token-service` → `iam-service`

**Files:**
- Modify: `services/token-service/src/issuer.rs` (or wherever `fetch_json(/iam/internal/users/:id/claims)` lives)
- Modify: `services/iam-service/src/main.rs` (allow `Audience::Any` on `/iam/internal/*` if not already)
- Modify: `services/token-service/src/config.rs` (if there's an `iam_base_url` env, set it to `http://iam-service:8081`)

**Interfaces:**
- Consumes: Task 3 (services up on `iam-service:8081`)
- Produces: `token-service` POST/GET to `http://iam-service:8081/iam/internal/...` carries `Authorization: Bearer <user-jwt>`; `iam-service` accepts that header value as an `Audience::Any` token

- [ ] **Step 1: Locate the call site**

Run:
```bash
grep -rn "iam-service" services/token-service/src/ --include="*.rs" | grep -i "fetch\|http\|reqwest\|client"
```
Expected: a single call site in `services/token-service/src/issuer.rs` (or whichever file the call lives in) that hits `/iam/internal/users/:id/claims`.

- [ ] **Step 2: Find the iam-service URL it uses today**

In that call site, identify the URL. If it points at `iam-service:9081` (the old sidecar port), change it to `iam-service:8081`. If it uses `${I_AM_BASE_URL}` or `iam_base_url`, ensure the env default reads `http://iam-service:8081`.

- [ ] **Step 3: Find the request-header builder**

The `reqwest::Client` (or hyper-based equivalent) call site passes headers. Locate where `Authorization` is set. Today it likely sets a service token (`aud: "service"`). Replace the construction.

If a helper exists in `token-service` like `iam_client::request(claims)`, edit it once. If not, inline-edit the single call site.

The replacement contract:

- Caller passes the original user JWT (`String` form, the same `Authorization` value the gateway injects).
- The HTTP request carries `Authorization: Bearer <user-jwt>` to iam-service.
- No `service_token` machinery is needed on this path anymore. Leave the function signature with `&str user_jwt` (or expand an existing function to take it).

- [ ] **Step 4: Update `iam-service` to allow `Audience::Any`**

Open `services/iam-service/src/main.rs` and find the route-level guard for `/iam/internal/*`. Look for `Audience` matching logic. Today it likely requires `Audience::Service`. Change to:

```rust
match claims.aud {
    Audience::Any | Audience::User | Audience::Service => Ok(()),
    _ => Err(...),
}
```

If `Audience::Any` does not exist as a variant, add it to the `Audience` enum (`services/iam-service/src/types.rs` or wherever it lives). The variant value is reserved for "any valid token from the gateway's signing key, regardless of `aud`"; it must NOT be honored on user-facing endpoints (`/iam/users/*`), only on `/iam/internal/*`. The route guard is the right place to scope that.

- [ ] **Step 5: Build**

```bash
cargo build -p token-service -p iam-service
```
Expected: clean compile. If `Audience::Any` doesn't compile, you missed adding the variant.

- [ ] **Step 6: Run targeted unit tests**

```bash
cargo test -p token-service --lib
cargo test -p iam-service --lib
```
Expected: existing tests pass. If new behavior is asserted, add a focused test:

```rust
#[tokio::test]
async fn fetch_iam_claims_forwards_authorization_header() {
    // mock iam-service: expects a request with Authorization: Bearer <jwt>
    // call token-service code path that hits /iam/internal/users/:id/claims
    // assert the request reached the mock with the header intact
}
```

Place this test next to the function. If the existing test suite doesn't include this scenario, write the minimal test above; otherwise rely on existing.

- [ ] **Step 7: Restart and smoke-test cross-service**

```bash
docker compose -f infrastructure/docker-compose.yml restart token-service iam-service
sleep 4
# From inside the token-service container, exercise the call path:
docker exec infrastructure-token-service-1 sh -c \
  'curl -sf http://iam-service:8081/health | head -3'
```
Expected: HTTP 200. The path that fetches claims requires running authorization machinery; if a `/scripts/dev-login` or similar fixture exists in the repo, use it. (If not, skipping Step 7 and trusting the unit tests is acceptable; flag the gap in the next retrospective.)

- [ ] **Step 8: Commit**

```bash
git add services/token-service/ services/iam-service/
git commit -m "dephase(token): pass Authorization header to iam-service, allow Audience::Any on /iam/internal/*"
```

---

## Task 5: Authorization passthrough — `notification-service` → `iam-service`

**Files:**
- Modify: `services/notification-service/src/main.rs` (or wherever the iam users lookup lives)
- Modify: `services/notification-service/src/config.rs` (set `iam_base_url` to `http://iam-service:8081`)

**Interfaces:**
- Consumes: Task 4 (`iam-service` now accepts `Audience::Any`)
- Produces: `notification-service` carries the JWT to `http://iam-service:8081/iam/internal/users/by-{role,dept}`

- [ ] **Step 1: Locate the call site**

Run:
```bash
grep -rn "iam-service\|iam_base_url\|by-role\|by-dept" services/notification-service/src/ --include="*.rs"
```
Expected: a single HTTP call to `/iam/internal/users/by-{role,dept}`.

- [ ] **Step 2: Apply the same edit pattern as Task 4**

- Change upstream URL to `http://iam-service:8081/iam/internal/users/by-...`
- Carry the user JWT in `Authorization: Bearer <jwt>`
- Drop any service-token / sidecar specific logic.

The notification-service call site may be a separate function from token-service's. Patch it independently; do not try to extract a shared library crate just for this.

- [ ] **Step 3: Build**

```bash
cargo build -p notification-service
```
Expected: clean.

- [ ] **Step 4: Targeted test**

```bash
cargo test -p notification-service --lib
```
Expected: existing tests pass. If users-lookup is tested, extend the mock to assert the header.

- [ ] **Step 5: Commit**

```bash
git add services/notification-service/
git commit -m "dephase(notification): jwt passthrough to iam-service for users lookup"
```

---

## Task 6: Strip `ConsulRegistrar` call sites from all 7 services

**Files:**
- Modify: `services/<each>/src/main.rs` (api-gateway, iam, token, notification, audit, prue_backend — 6 services that called `ConsulRegistrar::new`)
- Modify: `services/<each>/Cargo.toml` (drop `consul-registrar = { workspace = true }` dep)
- Modify: `Cargo.toml` (drop `members = [..., "libs/consul-registrar"]` line — but DO NOT delete the directory yet, that is Task 7)

**Interfaces:**
- Consumes: spec §5.2 row "services/<each>/src/main.rs"
- Produces: no `ConsulRegistrar` symbol referenced from any service `main.rs`

- [ ] **Step 1: Find all call sites**

Run:
```bash
grep -rn "ConsulRegistrar\|consul_registrar" services/ --include="*.rs"
```
Expected: 0 hits (Task 6 final state).

If `services/<each>/Cargo.toml` still has `consul-registrar = { workspace = true }`, this is also part of step.

- [ ] **Step 2: Inspect a representative call site**

```bash
grep -B2 -A30 "ConsulRegistrar::new" services/api-gateway/src/main.rs | head -50
```
Expect to see something like:

```rust
    let mut consul_registrar = ConsulRegistrar::new(
        config.service.id.clone(),
        config.service.name.clone(),
        config.service.address.clone(),
        config.service.port,
    );
    consul_registrar
        .with_upstream("iam-service", "iam-service", 9081)
        // ... possibly more `with_upstream` calls
        .with_sidecar_config("...", serde_json::json!({}))
        .register_service()
        .await?;
```

- [ ] **Step 3: Delete the block**

In each service's `main.rs`, delete:

1. The `use consul_registrar::...` import (if present)
2. The `let mut consul_registrar = ConsulRegistrar::new(...)` through `.register_service().await?;` block
3. Any related `config.service.*` fields whose only consumer was the registrar (drop these from `config.rs` too)

**Caution**: if any of these config fields are read elsewhere (logging, /health endpoint), keep them in `config.rs` but remove the registrar call.

- [ ] **Step 4: Drop the dep from each service `Cargo.toml`**

For each service, in `services/<name>/Cargo.toml`:

```toml
# DELETE this line:
consul-registrar = { workspace = true }
```

If a service had a `default-features` or `features = [...]` line elsewhere in consul-registrar usage, remove those too.

- [ ] **Step 5: Verify no warnings in `cargo build --workspace`**

```bash
cargo build --workspace
```
Expected: clean build. Warnings only if pre-existing; if any `consul_registrar` warning appears, you missed a service in Step 3.

- [ ] **Step 6: Run all service tests**

```bash
cargo test --workspace --lib
```
Expected: all green.

- [ ] **Step 7: Restart service stack and recheck Consul catalog**

```bash
docker compose -f infrastructure/docker-compose.yml restart api-gateway iam-service token-service notification-service audit-service prue_backend
sleep 8
docker exec infrastructure-consul-1 consul catalog services
```
Expected: same 9 services as in Task 3. The services are now in the catalog by registrator labels only, not by in-process `ConsulRegistrar`. If a service is missing, registrator didn't pick up its labels; inspect the container's labels.

- [ ] **Step 8: Commit**

```bash
git add services/ Cargo.toml
git commit -m "dephase(services): remove ConsulRegistrar call sites, drop service-side Consul registration"
```

---

## Task 7: Delete `libs/consul-registrar` crate

**Files:**
- Delete: `libs/consul-registrar/Cargo.toml`
- Delete: `libs/consul-registrar/src/lib.rs`
- Delete: `libs/consul-registrar/src/` (entire dir if multi-file)
- Delete: `libs/consul-registrar/tests/` (entire dir)
- Modify: `Cargo.toml` (remove `members = [..., "libs/consul-registrar"]` entry if still present)

**Interfaces:**
- Consumes: Task 6 (no service depends on `consul-registrar` anymore)
- Produces: workspace builds without `libs/consul-registrar`

- [ ] **Step 1: Confirm no callers**

```bash
cargo tree --workspace | grep consul-registrar
```
Expected: empty output. If anything remains, Task 6 is incomplete.

- [ ] **Step 2: Remove the directory**

Run:
```bash
rm -rf libs/consul-registrar
ls libs/
```
Expected: `consul-registrar` no longer in the listing. Confirm other libs intact.

- [ ] **Step 3: Remove from workspace `Cargo.toml`**

Open `Cargo.toml`. Find the `members = [...]` list under `[workspace]`. Remove the line `"libs/consul-registrar"` (and any sibling entry that pointed to it). Also remove any `[workspace.dependencies] consul-registrar = ...` block (if present).

- [ ] **Step 4: Build workspace**

```bash
cargo build --workspace
```
Expected: clean.

- [ ] **Step 5: Run tests**

```bash
cargo test --workspace --lib
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml
git commit -m "dephase(lib): delete libs/consul-registrar"
```

---

## Task 8: Docs cleanup — supersede phase5a, drop `scripts/consul-intentions.sh`, refresh `ai-context`

**Files:**
- Modify: `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md` (add banner)
- Modify: `docs/superpowers/plans/2026-06-29-phase5a-mtls-sidecar.md` (add banner)
- Modify: `docs/ai-context.md` (refresh)
- Modify: `docs/guides/mesh-service-onboarding.md` (replace or delete)
- Delete: `scripts/consul-intentions.sh` (chmod-only-diff from earlier session rolled into the git tree as a deletion)

**Interfaces:**
- Consumes: spec §11 (forward-looking documentation)
- Produces: documents reflect new architecture; future readers do not see mesh code as recommended

- [ ] **Step 1: Add superseded banner to phase5a spec**

Open `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md`. After the title line, add:

```markdown
> ⚠️ **Superseded by** `docs/superpowers/specs/2026-07-07-deprecate-consul-connect-use-registrator-design.md` (2026-07-07). The Consul Connect / mTLS / sidecar approach described here was rolled back in favor of direct HTTP + JWT passthrough and `gliderlabs/registrator` for service discovery. Retained for historical reference only; do not follow this document for new work.
```

- [ ] **Step 2: Add superseded banner to phase5a plan**

Same pattern in `docs/superpowers/plans/2026-06-29-phase5a-mtls-sidecar.md`:

```markdown
> ⚠️ **Superseded by** `docs/superpowers/plans/2026-07-07-deprecate-consul-connect.md` (2026-07-07). The consul-dataplane sidecar approach described here was rolled back. Retained for historical reference only.
```

- [ ] **Step 3: Refresh `docs/ai-context.md`**

Open `docs/ai-context.md`. Find these sections and update:

- **Service map** (if present): sidecars removed; add `registrator` as infra service.
- **Pitfall list** (if present): remove any "service must bind 127.0.0.1 because of sidecar" guidance — `SERVER_HOST=0.0.0.0` is now fine.
- **Architecture decisions log**: append an entry pointing at spec / plan.

Concrete wording for the ADR-style append (if the doc has such a section):

```markdown
### 2026-07-07 — Drop Consul Connect

**Context:** Phase 5-A introduced a Consul Connect mesh (`docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md`). Operational friction (dev-mode TLS mismatch, manual node pre-registration, image/command drift across 6 sidecars) and the project's `low-memory-micro-services` direction pushed for removal.

**Decision:** Replace mesh with `gliderlabs/registrator` (project-level container, mounted docker.sock, label-driven). Service-to-service authn shifts to JWT passthrough. Service registration moves from in-process `libs/consul-registrar` to compose labels.

**Consequences:** -400MB memory; service code no longer embeds Consul knowledge; production-tunneling path to be addressed in phase5b.

**Reference:** `docs/superpowers/specs/2026-07-07-deprecate-consul-connect-use-registrator-design.md`; implementation plan `docs/superpowers/plans/2026-07-07-deprecate-consul-connect.md`.
```

- [ ] **Step 4: Replace mesh-onboarding guide**

`docs/guides/mesh-service-onboarding.md` is no longer relevant. Pick one:

- **Option A (delete)**: `git rm docs/guides/mesh-service-onboarding.md`
- **Option B (rewrite)**: rename to `service-to-service-auth.md` and write a short guide (≤ 60 lines) covering: registrator labels; JWT passthrough; `Audience::Any` on internal endpoints.

Default to **Option A** — the spec left this as a follow-up. Document the deletion in the next retrospective.

- [ ] **Step 5: Delete `scripts/consul-intentions.sh`**

```bash
git rm scripts/consul-intentions.sh
ls scripts/
```
Expected: no `consul-intentions.sh` in the listing.

Note: the working tree may show `chmod +x` as the only "change"; this step removes the file outright.

- [ ] **Step 6: Confirm docs build / link sanity (if a docs checker exists)**

```bash
# If a docs build exists; otherwise skip.
ls scripts/ docs/
```

- [ ] **Step 7: Commit**

```bash
git add -A docs/ scripts/
git commit -m "dephase(docs): supersede phase5a, drop intentions.sh, refresh ai-context"
```

---

## Task 9: Sync `docker-example` with the new compose

**Files:**
- Modify: `docker-example/**/*Dockerfile*` (drop sidecar assumptions)
- Modify: `docker-example/docker-compose*.yml` (mirror Step 3 changes)

**Interfaces:**
- Consumes: spec §11 ("docker-example 同步")
- Produces: docker-example compose mirrors production; newcomers who copy it can spin up a working stack

- [ ] **Step 1: Inventory docker-example**

Run:
```bash
ls docker-example/
```
Inspect: is there a `docker-compose.yml`, per-service `Dockerfile`s, or a `services/` mirror?

- [ ] **Step 2: Mirror Step 3 changes into docker-example**

Apply the same edits from Task 3 to `docker-example/` if it has its own compose / configs:

- Remove sidecar services
- Flip ports
- Update gateway upstream URLs

If `docker-example/` only contains sample secrets / config that the compose mounts (not actual Dockerfiles/composes), Step 2 is a no-op — move to Step 3.

- [ ] **Step 3: Smoke-test if applicable**

If `docker-example` is runnable:

```bash
docker compose -f docker-example/docker-compose.yml --project-directory docker-example up -d
```
(Adjust command per the actual layout.)

If not runnable in this session, document the gap in commit and move on.

- [ ] **Step 4: Commit**

```bash
git add docker-example/
git commit -m "dephase(example): drop sidecars, mirror production compose"
```

---

## Acceptance criteria

All of the following must hold after Task 9:

1. `docker compose -f infrastructure/docker-compose.yml config` shows no `*-sidecar` service, no `connect { ... }` block, and 8 business services with `SERVICE_*` labels.
2. `docker exec infrastructure-consul-1 consul catalog services` lists 9 services (8 business + `consul`).
3. `docker exec infrastructure-consul-1 curl -sf http://iam-service:8081/health` returns 200.
4. `docker exec infrastructure-consul-1 curl -sf http://api-gateway:3000/health` returns 200.
5. `cargo test --workspace --lib` is green.
6. `node scripts/run-tests-docker.js` (or the relevant subset) is green for the 7 services.
7. `grep -rn ConsulRegistrar services/` returns no results.
8. `grep -rn consul-registrar Cargo.toml services/*/Cargo.toml` returns no results.
9. `scripts/consul-intentions.sh` is deleted.
10. `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md` carries the superseded banner.

---

## Execution-time risks and mitigations

| Risk | Detection | Mitigation |
|---|---|---|
| Registrator labels miss a service | `consul catalog services` shows fewer than 9 entries | `docker inspect <cont>` → labels section; `docker logs infrastructure-registrator-1` |
| Internal call path bypasses Authorization header | token-service tests fail with 401 | Re-verify Task 4 / Task 5 patches carry the header literal |
| `consul-init`'s `network_mode: service:consul` removal breaks something | docker compose ps shows consul-init still restarting | Confirm the `consul-init:` block was fully deleted (Step 1.3) |
| Port host-port collision on `8085:8081` / `8086:8082` | `bind: address already in use` | Check `/etc/services` + container listing |
| `Cargo.toml` workspace `members` list out of sync after Task 7 | `cargo build --workspace` errors on missing crate member | Re-confirm Task 7.3 deletion; do not delete `Cargo.toml` line by hand without re-rendering |
| Docs banner format drifts between spec/plan | Reviewer rejects commit message | Use the exact wording in Steps 8.1 / 8.2 verbatim |
| Working tree drift between task boundaries | `git status` shows stray changes mid-plan | Each task's final step is a `git commit` — never accumulate |

---

## Self-Review (post-draft, kept in plan for traceability)

- [x] **Spec coverage**: §2.1 → Tasks 1–3; §2.2 → Task 3; §2.4 → Tasks 7, 8; §3.2 → Task 2; §3.3 → Task 2; §4.1 → Tasks 4, 5; §4.2 → Tasks 4, 5; §5.2 → Tasks 3, 6, 7, 8; §6 → Task 7; §7 → Task 3; §8 → Tasks 1, 3, 9; §9 → spec-level ADR; §10 → Tasks 6–8; §11 → Tasks 8, 9.
- [x] **No placeholder patterns**: Each step has explicit code or commands. The only "see spec" reference is §6 / Task 7 which says "delete the entire crate" — that's a single shell command with no placeholder required.
- [x] **Type / method / port consistency across tasks**: `8081` (iam + config-service), `8082` (token + file-service), `8083` (notification), `8084` (audit), `8087` (prue_backend), `3000` (api-gateway). Label anchor matches across Tasks 2 and 3.
- [x] **Bite-sized steps**: Each step is one operation, 2–5 minutes; the only "large" Step is Task 3.6 (delete 6+ lines across files), which is unavoidable in an atomic cutover.
- [x] **One commit per task**: Step `git commit` lines confirm. No commit bundles multiple tasks.
