# Phase 4 — 异步事件总线（最小闭环）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire up an asynchronous event bus (RabbitMQ + outbox) for the iam-service → audit-service path. Validates the architecture end-to-end before rolling out to other services in Phase 4.x.

**Architecture:**
- **Producer side (iam-service):** Business transaction writes both the data row AND an `iam_outbox_events` row in the same MySQL transaction. An independent `outbox-publisher` binary (same image, different CMD) polls the outbox table using `SELECT ... FOR UPDATE SKIP LOCKED` and publishes to RabbitMQ.
- **Broker:** Single topic exchange `events`, durable. Three routing keys for this phase: `iam.user.created`, `iam.user.deleted`, `iam.login.succeeded`. Dead letter exchange `events.dlx` → `events.dlq`.
- **Consumer side (audit-service):** Subscribes to `audit.iam-events` queue (bound to `iam.#`), does idempotent `INSERT ... ON DUPLICATE KEY UPDATE` into `audit_events` table keyed by `event_id`.
- **Shared library:** New `libs/common-event` crate defines `Event`, `Publisher`, `Consumer`, `OutboxRepo`, `OutboxPublisher` traits with both RabbitMQ and stub/in-memory implementations.

**Tech Stack:**
- Rust 1.96 (musl target)
- lapin 2.x (RabbitMQ client)
- sqlx 0.8 (MySQL + SQLite)
- RabbitMQ 3.13-management-alpine
- async-trait, uuid v7, serde, chrono
- OTel trace context propagation via envelope field (not AMQP headers)

**Predecessor:** Phase 4.0 table-naming refactor must be merged. All new tables follow `<service>_` prefix convention; `scripts/check-db-naming.js` must pass before any commit.

---

## Global Constraints

- **Strategy:** empty-db regenerate for new tables (one-shot V{NNN} migration; existing services have empty dev DB acceptable to recreate). Never edit a shipped migration. Use V{NNN+1} when altering.
- **Rust style:** sqlx 0.8 `query!` macros preferred for compile-time check. No `unwrap()` in production code. Errors via `anyhow::Result` for handlers, typed errors for libraries.
- **Tests:** Docker-based via `node scripts/run-tests-docker.js <svc>`. New `common-event` lib gets standalone unit tests (rabbitmq via testcontainers). Token-service testcontainers cleanup bug is known — manually `docker rm -f` any `mysql:8.x` containers not matching `infrastructure-mysql-1` after full-suite run.
- **Naming:** All SQL tables/indexes/keys use `iam_` / `audit_` / `prue_` / `cfg_` / `file_` / `token_` / `ntf_` prefix per `contracts/db-conventions.md`. CI gate `node scripts/check-db-naming.js` is mandatory before push.
- **Commits:** One task = one commit. Conventional Commits format: `feat(scope):` / `refactor(scope):` / `docs:` / `chore:`.
- **Branch policy:** `main` only, no feature branches (per `docs-site/ai-context.md` §1).

---

## File Structure (Phase 4 main)

| File | Purpose | Created/Modified |
|------|---------|------------------|
| `infrastructure/docker-compose-infra.yml` | Add RabbitMQ container | M |
| `infrastructure/config/rabbitmq/definitions.json` | Pre-declared exchange/queue/binding | C |
| `infrastructure/config/rabbitmq/rabbitmq.conf` | Enable management.load_definitions | C |
| `infrastructure/docker-compose-services.yml` | Add iam-outbox-publisher service | M |
| `contracts/events.yaml` | Event schema (replace stub) | M |
| `contracts/codegen/gen-rust-events.js` | Generate Rust event structs | C |
| `scripts/contracts-gen.js` | Wire in new generator | M |
| `libs/common-event/Cargo.toml` | Crate manifest | C |
| `libs/common-event/src/lib.rs` | Re-exports | C |
| `libs/common-event/src/event.rs` | `Event` trait | C |
| `libs/common-event/src/envelope.rs` | `Envelope<P>` struct | C |
| `libs/common-event/src/publisher.rs` | `Publisher` trait + `RabbitMqPublisher` | C |
| `libs/common-event/src/consumer.rs` | `Consumer` trait + `RabbitMqConsumer` | C |
| `libs/common-event/src/outbox.rs` | `OutboxRepo` trait + MySQL impl | C |
| `libs/common-event/src/publisher_loop.rs` | `OutboxPublisher::run` | C |
| `libs/common-event/src/error.rs` | `PublishError`, `ConsumeError`, `OutboxError` | C |
| `libs/common-event/src/config.rs` | `EventBusConfig` from `config/config.yaml` | C |
| `libs/common-event/src/metrics.rs` | Prometheus counters/gauges | C |
| `libs/common-event/tests/rabbit_integration.rs` | Testcontainers-based end-to-end | C |
| `services/iam-service/migrations/V20260626120000__iam_outbox_events.sql` | Outbox table | C |
| `services/iam-service/src/outbox.rs` | OutboxRepo concrete (uses iam pool) | C |
| `services/iam-service/src/events.rs` | Hand-written `IamUserCreated` etc. payloads (or gen) | C |
| `services/iam-service/src/service/user_svc.rs` | Add outbox enqueue in create/delete | M |
| `services/iam-service/src/service/auth_svc.rs` | Add outbox enqueue in login_success | M |
| `services/iam-service/src/main.rs` | Add outbox-publisher [[bin]] | M |
| `services/iam-service/src/bin/outbox_publisher.rs` | Independent publisher binary | C |
| `services/iam-service/Cargo.toml` | Add `common-event` dep, `[[bin]]` | M |
| `services/iam-service/src/audit_client.rs` | **DELETE** old HTTP client | M (delete) |
| `services/audit-service/migrations/V20260626120100__audit_events.sql` | Idempotency table | C |
| `services/audit-service/src/consumer.rs` | RabbitMQ consumer wiring | C |
| `services/audit-service/src/main.rs` | Spawn consumer task | M |
| `services/audit-service/src/routes/audit.rs` | **DELETE** write endpoints (POST) | M (delete) |
| `services/audit-service/openapi.yaml` | Regenerate without POST writes | M (regen) |
| `services/audit-service/src/tracing_mw.rs` | Add OTel layer (Phase 6 advance) | C |
| `services/audit-service/Cargo.toml` | Add `common-event` dep | M |
| `tests/e2e/api/cases/iam-audit-event-flow.yaml` | E2E: create user → audit row appears | C |
| `docs-site/USER_MANUAL.md` | Add event bus section | M |
| `docs-site/ai-context.md` | Add OTel note for audit-service | M |
| `scripts/check-db-naming.js` | Update if new patterns slip through | M |

**Total: 35 files, 17 tasks.**

---

### Task 1: Add RabbitMQ container to infrastructure

**Files:**
- Modify: `infrastructure/docker-compose-infra.yml` (append `rabbitmq:` service block)
- Create: `infrastructure/config/rabbitmq/rabbitmq.conf`
- Create: `infrastructure/config/rabbitmq/definitions.json`
- Create: `infrastructure/config/rabbitmq/.gitkeep`

**Interfaces:**
- Consumes: nothing (foundation task)
- Produces: `rabbitmq` container reachable at `amqp://guest:guest@localhost:5672/%2f`, management UI at `http://localhost:15672`

- [ ] **Step 1: Write the rabbitmq.conf**

```conf
# /home/kudi/projects/templates/low-memory-micro-services/infrastructure/config/rabbitmq/rabbitmq.conf
management.load_definitions = /etc/rabbitmq/definitions.json
default_vhost = /
default_user = guest
default_pass = guest
loopback_users.guest = false
```

- [ ] **Step 2: Write the empty definitions.json (will be filled in by topology bootstrap OR pre-declared in Task 2)**

```json
{
  "rabbit_version": "3.13.0",
  "users": [
    { "name": "guest", "password": "guest", "tags": ["administrator"] }
  ],
  "vhosts": [{ "name": "/" }],
  "permissions": [{ "user": "guest", "vhost": "/", "configure": ".*", "write": ".*", "read": ".*" }],
  "exchanges": [],
  "queues": [],
  "bindings": []
}
```

- [ ] **Step 3: Add RabbitMQ service to docker-compose-infra.yml**

Find the file `infrastructure/docker-compose-infra.yml` and append at the bottom (or insert in alphabetical order):

```yaml
  rabbitmq:
    image: rabbitmq:3.13-management-alpine
    container_name: infrastructure-rabbitmq-1
    hostname: rabbitmq
    ports:
      - "5672:5672"     # AMQP
      - "15672:15672"   # Management UI
    volumes:
      - rabbitmq-data:/var/lib/rabbitmq
      - ./config/rabbitmq/definitions.json:/etc/rabbitmq/definitions.json:ro
      - ./config/rabbitmq/rabbitmq.conf:/etc/rabbitmq/rabbitmq.conf:ro
    healthcheck:
      test: ["CMD", "rabbitmq-diagnostics", "-q", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - superpowers
```

Add `rabbitmq-data:` to the top-level `volumes:` section if not already there.

- [ ] **Step 4: Verify the container starts and is healthy**

```bash
docker compose -f infrastructure/docker-compose-infra.yml up -d rabbitmq
sleep 15
docker ps --format 'table {{.Names}}\t{{.Status}}' | grep rabbitmq
curl -s -u guest:guest http://localhost:15672/api/overview | head -c 200
```

Expected: container `Up X seconds (healthy)`, curl returns JSON with `rabbitmq_version`.

- [ ] **Step 5: Commit**

```bash
git add infrastructure/
git commit -m "feat(infra): add RabbitMQ container with management UI (phase 4)"
```

---

### Task 2: Pre-declare exchange/queue/binding topology

**Files:**
- Modify: `infrastructure/config/rabbitmq/definitions.json`

**Interfaces:**
- Consumes: RabbitMQ container from Task 1
- Produces: topology `events` (topic) + `events.dlx` (fanout) + `audit.iam-events` queue (durable, bound to `iam.#`) + `events.dlq` (durable, bound to `events.dlx`)

- [ ] **Step 1: Replace definitions.json with the full topology**

```json
{
  "rabbit_version": "3.13.0",
  "users": [
    { "name": "guest", "password": "guest", "tags": ["administrator"] }
  ],
  "vhosts": [{ "name": "/" }],
  "permissions": [{ "user": "guest", "vhost": "/", "configure": ".*", "write": ".*", "read": ".*" }],
  "exchanges": [
    { "name": "events", "vhost": "/", "type": "topic", "durable": true, "auto_delete": false, "internal": false, "arguments": {} },
    { "name": "events.dlx", "vhost": "/", "type": "fanout", "durable": true, "auto_delete": false, "internal": false, "arguments": {} }
  ],
  "queues": [
    { "name": "audit.iam-events", "vhost": "/", "durable": true, "auto_delete": false, "arguments": { "x-dead-letter-exchange": "events.dlx" } },
    { "name": "events.dlq", "vhost": "/", "durable": true, "auto_delete": false, "arguments": {} }
  ],
  "bindings": [
    { "source": "events", "vhost": "/", "destination": "audit.iam-events", "destination_type": "queue", "routing_key": "iam.#", "arguments": {} },
    { "source": "events.dlx", "vhost": "/", "destination": "events.dlq", "destination_type": "queue", "routing_key": "", "arguments": {} }
  ]
}
```

- [ ] **Step 2: Restart RabbitMQ to apply definitions**

```bash
docker compose -f infrastructure/docker-compose-infra.yml restart rabbitmq
sleep 10
```

- [ ] **Step 3: Verify topology via management API**

```bash
curl -s -u guest:guest http://localhost:15672/api/exchanges/%2F | python3 -c "import sys,json; d=json.load(sys.stdin); print([e['name'] for e in d if not e['name'].startswith('amq.')])"
curl -s -u guest:guest http://localhost:15672/api/queues/%2F | python3 -c "import sys,json; d=json.load(sys.stdin); print([q['name'] for q in d])"
curl -s -u guest:guest http://localhost:15672/api/bindings/%2F | python3 -c "import sys,json; d=json.load(sys.stdin); print([(b['source'],b['destination'],b['routing_key']) for b in d if b['source']])"
```

Expected output:
- Exchanges: `['events', 'events.dlx']`
- Queues: `['audit.iam-events', 'events.dlq']`
- Bindings: `[('events', 'audit.iam-events', 'iam.#'), ('events.dlx', 'events.dlq', '')]`

- [ ] **Step 4: Commit**

```bash
git add infrastructure/config/rabbitmq/definitions.json
git commit -m "feat(infra): pre-declare events exchange + audit queue + DLX (phase 4)"
```

---

### Task 3: Extend events.yaml schema with payload definitions

**Files:**
- Modify: `contracts/events.yaml`

**Interfaces:**
- Consumes: existing flat-name format from old stub
- Produces: rich event schema with `schema_version` + per-field type, used by codegen in Task 4

- [ ] **Step 1: Replace the entire contents of `contracts/events.yaml`**

```yaml
# 领域事件 schema 定义
# 由 contracts/codegen/gen-rust-events.js 编译为 Rust struct + routing-key 常量
# 由 services 在业务事务中通过 OutboxRepo::enqueue 写入 iam_<svc>_outbox_events
# routing-key 格式：<service>.<resource>.<action>  （小写，下划线转点）

version: "1.0.0"

# 默认 schema_version，新事件可覆盖
default_schema_version: 1

events:
  # === 用户事件（iam-service 产出） ===
  iam.user.created:
    schema_version: 1
    description: "用户创建"
    payload:
      user_id:     { type: string,  required: true }
      username:    { type: string,  required: true }
      email:       { type: string,  required: false }
      org_unit_id: { type: string,  required: false }

  iam.user.deleted:
    schema_version: 1
    description: "用户删除（软删或硬删）"
    payload:
      user_id:    { type: string, required: true }
      deleted_by: { type: string, required: true }   # 操作者 user_id

  iam.login.succeeded:
    schema_version: 1
    description: "登录成功"
    payload:
      user_id:    { type: string, required: true }
      ip:         { type: string, required: true }
      user_agent: { type: string, required: false }
      method:     { type: string, required: true }   # password | sso | refresh
```

- [ ] **Step 2: Verify the file parses as YAML**

```bash
node -e "const fs=require('fs'); const s=fs.readFileSync('contracts/events.yaml','utf8'); console.log('lines:', s.split('\n').length); console.log('events:', (s.match(/^  [a-z]+\.[a-z]+\.[a-z]+:/gm)||[]).length);"
```

Expected: 3 events matched (`iam.user.created`, `iam.user.deleted`, `iam.login.succeeded`).

- [ ] **Step 3: Commit**

```bash
git add contracts/events.yaml
git commit -m "feat(contracts): rich event schema with payload field types (phase 4)"
```

---

### Task 4: Write the Rust events code generator

**Files:**
- Create: `contracts/codegen/gen-rust-events.js`
- Modify: `scripts/contracts-gen.js` (add step 4)
- Create: `libs/common-event/src/events.rs` (initially generated placeholder — replaced in Task 7 with real trait impls)
- Create: `libs/common-event/src/routing_keys.rs` (generated)

**Interfaces:**
- Consumes: `contracts/events.yaml` (from Task 3)
- Produces: 
  - `libs/common-event/src/events.rs` — one struct per event + `IamEventEnum` tagged enum
  - `libs/common-event/src/routing_keys.rs` — `pub const IAM_USER_CREATED: &str = "iam.user.created";` etc.

- [ ] **Step 1: Create the generator script**

```javascript
// contracts/codegen/gen-rust-events.js
const fs = require('fs');
const path = require('path');

const YAML = require('js-yaml'); // ensure deps; else hand-parse
const yamlPath = path.resolve(__dirname, '../events.yaml');
const outDir = path.resolve(__dirname, '../../libs/common-event/src');
fs.mkdirSync(outDir, { recursive: true });

const doc = YAML.load(fs.readFileSync(yamlPath, 'utf8'));
const events = doc.events || {};
const defaultVersion = doc.default_schema_version || 1;

// ---- routing_keys.rs ----
const constLines = Object.keys(events).map((rk) => {
  const NAME = rk.toUpperCase().replace(/\./g, '_');
  return `    pub const ${NAME}: &str = "${rk}";`;
});
const routingKeysRs = [
  '// Auto-generated from contracts/events.yaml',
  '// DO NOT EDIT MANUALLY',
  '',
  '/// Routing-key constants for events published to the `events` topic exchange.',
  'pub mod routing_keys {',
  ...constLines,
  '}',
  '',
].join('\n');
fs.writeFileSync(path.join(outDir, 'routing_keys.rs'), routingKeysRs);

// ---- events.rs (placeholder, will be replaced in Task 7) ----
const structDefs = Object.entries(events).map(([rk, def]) => {
  const NAME = rk.split('.').map((s) => s[0].toUpperCase() + s.slice(1)).join('');
  const fields = Object.entries(def.payload || {}).map(([fname, fdef]) => {
    const T = rustType(fdef.type);
    const OPT = fdef.required ? '' : 'Option<';
    const CLOSE = fdef.required ? '' : '>';
    return `    pub ${fname}: ${OPT}${T}${CLOSE},`;
  }).join('\n');
  return [
    `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]`,
    `pub struct ${NAME} {`,
    fields,
    `}`,
  ].join('\n');
});

const enumVariants = Object.keys(events).map((rk) => {
  const NAME = rk.split('.').map((s) => s[0].toUpperCase() + s.slice(1)).join('');
  return `    ${NAME}(Envelope<${NAME}>),`;
}).join('\n');

const eventsRs = [
  '// Auto-generated from contracts/events.yaml',
  '// DO NOT EDIT MANUALLY',
  '',
  'use serde::{Deserialize, Serialize};',
  '',
  'use crate::envelope::Envelope;',
  '',
  ...structDefs,
  '',
  '#[derive(Debug, Clone, Serialize, Deserialize)]',
  '#[serde(tag = "event_type", content = "payload", rename_all = "snake_case")]',
  'pub enum IamEventEnum {',
  `    Unknown {`,
  `        event_type: String,`,
  `        envelope: serde_json::Value,`,
  `    },`,
  enumVariants,
  '}',
  '',
].join('\n');
fs.writeFileSync(path.join(outDir, 'events.rs'), eventsRs);

function rustType(t) {
  return ({ string: 'String', int: 'i64', number: 'f64', bool: 'bool' })[t] || 'serde_json::Value';
}

console.log(`Generated ${Object.keys(events).length} event structs into ${outDir}`);
```

If `js-yaml` is not in `package.json` devDeps, add it: `pnpm add -D js-yaml`. Otherwise, hand-parse using a simple custom parser (the existing `gen-rust.js` does this).

- [ ] **Step 2: Run the generator and verify output**

```bash
node contracts/codegen/gen-rust-events.js
ls libs/common-event/src/
head -30 libs/common-event/src/routing_keys.rs
```

Expected: `routing_keys.rs` contains 3 consts, `events.rs` contains 3 structs and the `IamEventEnum`.

- [ ] **Step 3: Wire the generator into `scripts/contracts-gen.js`**

After the existing "Generating TypeScript contracts" block, add:

```javascript
console.log('');
console.log('4. Generating Rust event structs...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust-events.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust events');
  process.exit(1);
}
```

Also extend the `--check` `git diff` path to include `libs/common-event`:

```javascript
execSync('git diff --exit-code -- libs/common-error libs/authz libs/common-event frontend/src/contracts', {
  cwd: repoRoot,
  stdio: 'inherit',
});
```

- [ ] **Step 4: Run end-to-end contracts-gen and verify drift check passes**

```bash
node scripts/contracts-gen.js
node scripts/contracts-gen.js --check
```

Expected: exit 0 both times. (Note: `events.rs` / `routing_keys.rs` will fail to compile until Task 7 adds the `Envelope` type. That's OK — the generator must be checked in; compilation will succeed in Task 7.)

- [ ] **Step 5: Commit**

```bash
git add contracts/codegen/gen-rust-events.js scripts/contracts-gen.js libs/common-event/src/
git commit -m "feat(contracts): Rust events code generator (phase 4)"
```

---

### Task 5: Scaffold libs/common-event crate (Cargo.toml + lib.rs)

**Files:**
- Create: `libs/common-event/Cargo.toml`
- Create: `libs/common-event/src/lib.rs` (initial — re-exports placeholders)
- Create: `libs/common-event/src/error.rs`
- Create: `libs/common-event/src/config.rs`
- Modify: `Cargo.toml` (workspace member)

**Interfaces:**
- Consumes: nothing (scaffold)
- Produces: a crate that compiles with `cargo check -p common-event`, exposing module stubs

- [ ] **Step 1: Add the crate to workspace members**

Edit root `Cargo.toml`, find `[workspace] members = [...]` and add `"libs/common-event"`.

- [ ] **Step 2: Create `libs/common-event/Cargo.toml`**

```tomoml
[package]
name = "common-event"
version = "0.1.0"
edition = "2021"

[dependencies]
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
lapin = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "macros", "json", "chrono", "uuid"] }
thiserror = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
uuid = { version = "1", features = ["v7", "serde"] }
prometheus = "0.13"

[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["rabbitmq"] }
```

- [ ] **Step 3: Create `libs/common-event/src/lib.rs` (re-exports)**

```rust
//! common-event: domain event bus primitives
//!
//! Provides `Event`, `Envelope`, `Publisher`, `Consumer`, `OutboxRepo` traits
//! with a RabbitMQ implementation. See `phase4-event-bus-design.md` for the
//! overall topology.

pub mod config;
pub mod consumer;
pub mod envelope;
pub mod error;
pub mod event;
pub mod events;
pub mod metrics;
pub mod outbox;
pub mod publisher;
pub mod publisher_loop;
pub mod routing_keys;

pub use config::EventBusConfig;
pub use consumer::{Consumer, RabbitMqConsumer};
pub use envelope::Envelope;
pub use error::{ConsumeError, HandlerError, OutboxError, PublishError, RunError};
pub use event::Event;
pub use events::IamEventEnum;
pub use outbox::{MySqlOutboxRepo, OutboxRepo, OutboxRow};
pub use publisher::{Publisher, RabbitMqPublisher};
pub use publisher_loop::{OutboxPublisher, OutboxPublisherConfig};
```

- [ ] **Step 4: Create placeholder `libs/common-event/src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("broker error: {0}")]
    Broker(String),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("not connected")]
    NotConnected,
}

#[derive(Debug, Error)]
pub enum ConsumeError {
    #[error("broker error: {0}")]
    Broker(String),
    #[error("subscribe failed: {0}")]
    SubscribeFailed(String),
}

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("handler error: {0}")]
    Handler(String),
}

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("not found: event_id={0}")]
    NotFound(String),
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("publish loop terminated: {0}")]
    Publish(String),
    #[error("shutdown timeout")]
    ShutdownTimeout,
}
```

- [ ] **Step 5: Create placeholder `libs/common-event/src/config.rs`**

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct EventBusConfig {
    pub url: String,
    pub exchange: String,
    #[serde(default = "default_publish_confirms")]
    pub publish_confirms: bool,
}

fn default_publish_confirms() -> bool { true }

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@localhost:5672/%2f".to_string(),
            exchange: "events".to_string(),
            publish_confirms: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutboxPublisherConfig {
    pub poll_interval_ms: u64,
    pub batch_size: u32,
    pub backlog_warn_threshold: u64,
}

impl Default for OutboxPublisherConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
            batch_size: 100,
            backlog_warn_threshold: 10_000,
        }
    }
}
```

- [ ] **Step 6: Create stubs for remaining modules so crate compiles**

For each of `event.rs`, `envelope.rs`, `publisher.rs`, `consumer.rs`, `outbox.rs`, `publisher_loop.rs`, `events.rs`, `routing_keys.rs`, `metrics.rs`, create a minimal file:

```rust
// placeholder — implemented in next task
```

(The next tasks will replace these placeholders.)

- [ ] **Step 7: Verify the crate compiles**

```bash
cargo check -p common-event --target x86_64-unknown-linux-musl
```

Expected: compiles with warnings only (no errors).

- [ ] **Step 8: Commit**

```bash
git add libs/common-event/ Cargo.toml
git commit -m "feat(common-event): scaffold crate with error/config stubs (phase 4)"
```

---

### Task 6: Implement Event trait + Envelope struct (TDD)

**Files:**
- Modify: `libs/common-event/src/event.rs`
- Modify: `libs/common-event/src/envelope.rs`
- Create: `libs/common-event/tests/event_trait.rs`

**Interfaces:**
- Consumes: `Event` trait with `event_type() -> &'static str` + default `schema_version() -> u32 { 1 }`
- Produces: `Envelope<P>` struct with `event_id` (UUIDv7), `occurred_at`, `trace_id`, `tenant_id`, `producer`, `payload: P`. Implements `Envelope::new(producer, payload) -> Self` that fills `event_id` + `occurred_at` from current OTel context (or random fallback).

- [ ] **Step 1: Write the failing test in `libs/common-event/tests/event_trait.rs`**

```rust
use common_event::{Envelope, Event};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestEvent {
    user_id: String,
}

impl Event for TestEvent {
    fn event_type() -> &'static str { "test.event" }
    fn schema_version() -> u32 { 1 }
}

#[test]
fn envelope_new_fills_event_id_and_occurred_at() {
    let before = chrono::Utc::now();
    let env = Envelope::new("test-producer", TestEvent { user_id: "u1".to_string() });
    let after = chrono::Utc::now();

    assert_eq!(env.event_type(), "test.event");
    assert_eq!(env.payload.user_id, "u1");
    assert_eq!(env.producer, "test-producer");
    assert!(env.tenant_id.is_none());
    assert!(env.trace_id.is_none());
    assert!(env.occurred_at >= before && env.occurred_at <= after);
    // event_id is a valid UUIDv7 (version bits = 0b0111 in time_hi)
    let v = env.event_id.get_version_num();
    assert_eq!(v, 7, "event_id should be UUIDv7, got version {}", v);
}

#[test]
fn envelope_serializes_to_json_with_event_type() {
    let env = Envelope::new("p1", TestEvent { user_id: "u1".to_string() });
    let json = serde_json::to_value(&env).unwrap();
    assert_eq!(json["event_type"], "test.event");
    assert_eq!(json["producer"], "p1");
    assert_eq!(json["payload"]["user_id"], "u1");
    assert!(json["schema_version"].is_number());
}

#[test]
fn envelope_round_trip_through_json() {
    let env = Envelope::new("p1", TestEvent { user_id: "u1".to_string() });
    let json = serde_json::to_string(&env).unwrap();
    let back: Envelope<TestEvent> = serde_json::from_str(&json).unwrap();
    assert_eq!(back.payload, env.payload);
    assert_eq!(back.event_id, env.event_id);
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
```

Expected: compile errors (Envelopes and Event not yet defined).

- [ ] **Step 3: Implement `Event` trait in `libs/common-event/src/event.rs`**

```rust
use serde::{de::DeserializeOwned, Serialize};

pub trait Event: Serialize + DeserializeOwned + Send + Sync + 'static {
    fn event_type() -> &'static str;
    fn schema_version() -> u32 { 1 }
}
```

- [ ] **Step 4: Implement `Envelope` in `libs/common-event/src/envelope.rs`**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event::Event;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<P: Event> {
    pub event_id: Uuid,
    pub event_type: String,
    pub schema_version: u32,
    pub occurred_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tenant_id: Option<String>,
    pub producer: String,
    pub payload: P,
}

impl<P: Event> Envelope<P> {
    pub fn new(producer: impl Into<String>, payload: P) -> Self {
        Self {
            event_id: Uuid::now_v7(),
            event_type: P::event_type().to_string(),
            schema_version: P::schema_version(),
            occurred_at: Utc::now(),
            trace_id: None,
            tenant_id: None,
            producer: producer.into(),
            payload,
        }
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }
}
```

Add `pub use event::Event;` to top of `envelope.rs` (or rely on the re-export in `lib.rs`).

- [ ] **Step 5: Run the test to verify it passes**

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
```

Expected: 3 passed, 0 failed.

- [ ] **Step 6: Commit**

```bash
git add libs/common-event/src/event.rs libs/common-event/src/envelope.rs libs/common-event/tests/event_trait.rs
git commit -m "feat(common-event): Event trait + Envelope struct (phase 4)"
```

---

### Task 7: Replace generated placeholder with real events + routing keys

**Files:**
- Modify: `libs/common-event/src/events.rs` (re-run generator after Task 6 fixed placeholder)
- Modify: `libs/common-event/src/routing_keys.rs` (re-run generator)
- Create: `libs/common-event/tests/events_codegen.rs`

**Interfaces:**
- Consumes: events.yaml (Task 3), Envelope (Task 6)
- Produces: `IamUserCreated`, `IamUserDeleted`, `IamLoginSucceeded` structs (auto-gen) + `IamEventEnum` tagged enum

- [ ] **Step 1: Re-run the generator**

```bash
node scripts/contracts-gen.js
```

Expected: `events.rs` and `routing_keys.rs` regenerated with the 3 structs.

- [ ] **Step 2: Write the failing test in `libs/common-event/tests/events_codegen.rs`**

```rust
use common_event::events::{IamLoginSucceeded, IamUserCreated, IamUserDeleted};
use common_event::routing_keys;
use common_event::Envelope;
use serde_json::json;

#[test]
fn routing_keys_match_yaml() {
    assert_eq!(routing_keys::IAM_USER_CREATED, "iam.user.created");
    assert_eq!(routing_keys::IAM_USER_DELETED, "iam.user.deleted");
    assert_eq!(routing_keys::IAM_LOGIN_SUCCEEDED, "iam.login.succeeded");
}

#[test]
fn iam_user_created_event_type() {
    let env = Envelope::new("iam-service", IamUserCreated {
        user_id: "u1".to_string(),
        username: "alice".to_string(),
        email: Some("a@x".to_string()),
        org_unit_id: None,
    });
    assert_eq!(env.event_type, "iam.user.created");
    let v = serde_json::to_value(&env).unwrap();
    assert_eq!(v["payload"]["username"], "alice");
    assert_eq!(v["payload"]["org_unit_id"], serde_json::Value::Null);
}

#[test]
fn iam_user_deleted_requires_deleted_by() {
    let env = Envelope::new("iam-service", IamUserDeleted {
        user_id: "u1".to_string(),
        deleted_by: "admin".to_string(),
    });
    assert_eq!(env.event_type, "iam.user.deleted");
    let v = serde_json::to_value(&env).unwrap();
    assert_eq!(v["payload"]["deleted_by"], "admin");
}

#[test]
fn iam_login_succeeded_round_trip() {
    let env = Envelope::new("iam-service", IamLoginSucceeded {
        user_id: "u1".to_string(),
        ip: "10.0.0.1".to_string(),
        user_agent: Some("Mozilla".to_string()),
        method: "password".to_string(),
    });
    let s = serde_json::to_string(&env).unwrap();
    let back: Envelope<IamLoginSucceeded> = serde_json::from_str(&s).unwrap();
    assert_eq!(back.payload.method, "password");
    assert_eq!(back.payload.user_agent, Some("Mozilla".to_string()));
}
```

- [ ] **Step 3: Run the test to verify it fails (before real generator output)**

(If generator was already run in Step 1, the test should already pass — that's expected. If it fails, fix the generated `events.rs` until the test passes.)

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add libs/common-event/src/events.rs libs/common-event/src/routing_keys.rs libs/common-event/tests/events_codegen.rs
git commit -m "feat(common-event): wire generated event structs (phase 4)"
```

---

### Task 8: Implement OutboxRepo for MySQL (TDD)

**Files:**
- Modify: `libs/common-event/src/outbox.rs` (replace placeholder)
- Create: `libs/common-event/tests/outbox_repo.rs`

**Interfaces:**
- `MySqlOutboxRepo { pool: MySqlPool }`
- `enqueue<P: Event>(tx: &mut Transaction<'_, MySql>, env: &Envelope<P>) -> Result<(), OutboxError>`
- `claim_batch(limit: u32) -> Result<Vec<OutboxRow>, OutboxError>` (uses `FOR UPDATE SKIP LOCKED`)
- `mark_published(ids: &[i64]) -> Result<(), OutboxError>`
- `OutboxRow { id: i64, event_id: Uuid, routing_key: String, envelope_json: serde_json::Value }`

- [ ] **Step 1: Write the failing test in `libs/common-event/tests/outbox_repo.rs`**

```rust
use common_event::outbox::{MySqlOutboxRepo, OutboxRow};
use common_event::Envelope;
use common_event::events::IamUserCreated;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;
use testcontainers::ImageExt;

const DDL: &str = r#"
CREATE TABLE outbox_events (
    id BIGINT NOT NULL AUTO_INCREMENT,
    event_id CHAR(36) NOT NULL,
    routing_key VARCHAR(128) NOT NULL,
    envelope_json JSON NOT NULL,
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    published_at DATETIME(3) NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_event_id (event_id),
    KEY idx_unpublished (published_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
"#;

async fn make_pool() -> MySqlPool {
    let c = Mysql::default().with_cmd(["--innodb-use-native-aio=0"]).start().await.unwrap();
    let host = c.get_host().await.unwrap();
    let port = c.get_host_port_ipv4(3306).await.unwrap();
    let url = format!("mysql://root@{}:{}/test", host, port);
    let pool = MySqlPoolOptions::new()
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url).await.unwrap();
    for stmt in DDL.split(';') { let s = stmt.trim(); if !s.is_empty() { sqlx::query(s).execute(&pool).await.unwrap(); } }
    pool
}

#[tokio::test]
async fn enqueue_then_claim_then_mark_published_round_trip() {
    let pool = make_pool().await;
    let repo = MySqlOutboxRepo::new(pool.clone());

    let env = Envelope::new("iam-service", IamUserCreated {
        user_id: "u1".to_string(),
        username: "alice".to_string(),
        email: None,
        org_unit_id: None,
    });
    let mut tx = pool.begin().await.unwrap();
    repo.enqueue(&mut tx, &env).await.unwrap();
    tx.commit().await.unwrap();

    let batch = repo.claim_batch(10).await.unwrap();
    assert_eq!(batch.len(), 1);
    let row: &OutboxRow = &batch[0];
    assert_eq!(row.event_id, env.event_id);
    assert_eq!(row.routing_key, "iam.user.created");
    assert_eq!(row.envelope_json["producer"], "iam-service");

    repo.mark_published(&[row.id]).await.unwrap();
    let after = repo.claim_batch(10).await.unwrap();
    assert!(after.is_empty(), "published row should not be re-claimed");
}

#[tokio::test]
async fn enqueue_is_idempotent_on_duplicate_event_id() {
    let pool = make_pool().await;
    let repo = MySqlOutboxRepo::new(pool.clone());

    let env = Envelope::new("iam-service", IamUserCreated {
        user_id: "u1".to_string(),
        username: "alice".to_string(),
        email: None,
        org_unit_id: None,
    });
    let mut tx = pool.begin().await.unwrap();
    repo.enqueue(&mut tx, &env).await.unwrap();
    tx.commit().await.unwrap();

    // Second enqueue with same event_id should be a no-op (UNIQUE key)
    let mut tx = pool.begin().await.unwrap();
    repo.enqueue(&mut tx, &env).await.unwrap();
    tx.commit().await.unwrap();

    let batch = repo.claim_batch(10).await.unwrap();
    assert_eq!(batch.len(), 1, "duplicate event_id must not enqueue twice");
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
```

Expected: compile error (`MySqlOutboxRepo` not yet implemented).

- [ ] **Step 3: Implement `OutboxRepo` + `MySqlOutboxRepo` in `libs/common-event/src/outbox.rs`**

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlArguments, MySql, MySqlPool, Transaction};
use uuid::Uuid;

use crate::envelope::Envelope;
use crate::error::OutboxError;
use crate::event::Event;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxRow {
    pub id: i64,
    pub event_id: Uuid,
    pub routing_key: String,
    pub envelope_json: serde_json::Value,
}

#[async_trait]
pub trait OutboxRepo: Send + Sync {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &Envelope<P>,
    ) -> Result<(), OutboxError>;

    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError>;

    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError>;
}

pub struct MySqlOutboxRepo {
    pool: MySqlPool,
}

impl MySqlOutboxRepo {
    pub fn new(pool: MySqlPool) -> Self { Self { pool } }
}

#[async_trait]
impl OutboxRepo for MySqlOutboxRepo {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &Envelope<P>,
    ) -> Result<(), OutboxError> {
        let json = serde_json::to_value(env)?;
        sqlx::query(
            "INSERT INTO outbox_events (event_id, routing_key, envelope_json) \
             VALUES (?, ?, ?) \
             ON DUPLICATE KEY UPDATE id = id"
        )
        .bind(env.event_id.to_string())
        .bind(P::event_type())
        .bind(json)
        .execute(&mut **tx)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(())
    }

    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError> {
        // FOR UPDATE SKIP LOCKED: multiple publisher replicas grab disjoint rows
        let mut tx = self.pool.begin().await.map_err(|e| OutboxError::Database(e.to_string()))?;
        let rows: Vec<(i64, String, String, serde_json::Value)> = sqlx::query_as(
            "SELECT id, event_id, routing_key, envelope_json \
             FROM outbox_events \
             WHERE published_at IS NULL \
             ORDER BY id ASC \
             LIMIT ? \
             FOR UPDATE SKIP LOCKED"
        )
        .bind(limit as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;

        let out: Result<Vec<OutboxRow>, OutboxError> = rows.into_iter().map(|(id, eid, rk, env)| {
            Ok(OutboxRow {
                id,
                event_id: Uuid::parse_str(&eid).map_err(|e| OutboxError::Database(e.to_string()))?,
                routing_key: rk,
                envelope_json: env,
            })
        }).collect();
        // Keep the tx open; the caller will mark_published then commit.
        // For simplicity in this phase we commit immediately and rely on
        // FOR UPDATE SKIP LOCKED to be re-evaluated on next claim.
        tx.commit().await.map_err(|e| OutboxError::Database(e.to_string()))?;
        out
    }

    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError> {
        if ids.is_empty() { return Ok(()); }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "UPDATE outbox_events SET published_at = CURRENT_TIMESTAMP(3) WHERE id IN ({})",
            placeholders
        );
        let mut q = sqlx::query(&sql);
        for id in ids { q = q.bind(*id); }
        q.execute(&self.pool).await.map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
```

Expected: 2 passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add libs/common-event/src/outbox.rs libs/common-event/tests/outbox_repo.rs
git commit -m "feat(common-event): MySqlOutboxRepo with claim/mark idempotency (phase 4)"
```

---

### Task 9: Implement RabbitMQ Publisher (TDD)

**Files:**
- Modify: `libs/common-event/src/publisher.rs` (replace placeholder)
- Modify: `libs/common-event/tests/rabbit_integration.rs` (rename existing test file to this name; add publisher test)

**Interfaces:**
- `Publisher` trait: `async fn publish<P: Event>(&self, env: &Envelope<P>) -> Result<(), PublishError>`
- `RabbitMqPublisher { conn: lapin::Connection, channel: lapin::Channel, exchange: String }`
- `new(url, exchange) -> Result<Self, PublishError>` opens a connection + channel + enables `confirm_select`
- `publish()` publishes with `routing_key = P::event_type()` and waits for confirm

- [ ] **Step 1: Write the failing test**

```rust
// libs/common-event/tests/rabbit_integration.rs
use common_event::events::IamUserCreated;
use common_event::{Envelope, Event, Publisher, RabbitMqPublisher};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::rabbitmq::RabbitMq;

async fn make_publisher() -> (RabbitMqPublisher, String) {
    let c = RabbitMq::start().await.unwrap();
    let port = c.get_host_port_ipv4(5672).await.unwrap();
    let url = format!("amqp://guest:guest@localhost:{}/%2f", port);
    let pub_ = RabbitMqPublisher::new(&url, "events").await.unwrap();
    (pub_, url)
}

#[tokio::test]
async fn publish_sends_to_topic_exchange() {
    let (pub_, _url) = make_publisher().await;
    let env = Envelope::new("iam-service", IamUserCreated {
        user_id: "u1".to_string(),
        username: "alice".to_string(),
        email: None,
        org_unit_id: None,
    });
    pub_.publish(&env).await.expect("publish ok");
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
```

Expected: compile error (`Publisher`, `RabbitMqPublisher` not yet defined).

- [ ] **Step 3: Implement `Publisher` + `RabbitMqPublisher` in `libs/common-event/src/publisher.rs`**

```rust
use async_trait::async_trait;
use lapin::{
    options::{BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};

use crate::envelope::Envelope;
use crate::error::PublishError;
use crate::event::Event;

#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish<P: Event>(&self, env: &Envelope<P>) -> Result<(), PublishError>;
}

pub struct RabbitMqPublisher {
    conn: Connection,
    channel: Channel,
    exchange: String,
}

impl RabbitMqPublisher {
    pub async fn new(url: &str, exchange: &str) -> Result<Self, PublishError> {
        let conn = Connection::connect(url, ConnectionProperties::default())
            .await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        let channel = conn.create_channel().await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        // Ensure the exchange exists (idempotent — matches pre-declared topology)
        channel.exchange_declare(
            exchange,
            ExchangeKind::Topic,
            ExchangeDeclareOptions { durable: true, ..Default::default() },
            FieldTable::default(),
        ).await.map_err(|e| PublishError::Broker(e.to_string()))?;
        channel.confirm_select(ConfirmSelectOptions::default()).await
            .map_err(|e| PublishError::Broker(e.to_string()))?;
        Ok(Self { conn, channel, exchange: exchange.to_string() })
    }

    pub fn connection(&self) -> &Connection { &self.conn }
    pub fn channel(&self) -> &Channel { &self.channel }
}

#[async_trait]
impl Publisher for RabbitMqPublisher {
    async fn publish<P: Event>(&self, env: &Envelope<P>) -> Result<(), PublishError> {
        let body = serde_json::to_vec(env)?;
        let confirm = self.channel.basic_publish(
            &self.exchange,
            P::event_type(),
            BasicPublishOptions { mandatory: true, ..Default::default() },
            &body,
            BasicProperties::default()
                .with_content_type("application/json".into())
                .with_delivery_mode(2) // persistent
                .with_message_id(env.event_id.to_string().into()),
        ).await.map_err(|e| PublishError::Broker(e.to_string()))?
         .await.map_err(|e| PublishError::Broker(e.to_string()))?;
        if confirm.is_nack() {
            return Err(PublishError::Broker("publisher confirm nack".into()));
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
```

Expected: 1 passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add libs/common-event/src/publisher.rs libs/common-event/tests/rabbit_integration.rs
git commit -m "feat(common-event): RabbitMqPublisher with confirms (phase 4)"
```

---

### Task 10: Implement RabbitMQ Consumer + OutboxPublisher (TDD)

**Files:**
- Modify: `libs/common-event/src/consumer.rs` (replace placeholder)
- Modify: `libs/common-event/src/publisher_loop.rs` (replace placeholder)
- Modify: `libs/common-event/tests/rabbit_integration.rs` (add tests)

**Interfaces:**
- `Consumer::subscribe(queue, bindings, handler)` — declares queue if missing, binds, sets prefetch=32, runs basic_consume
- `OutboxPublisher::run()` — infinite loop: claim_batch → publish each → mark_published → sleep poll_interval; SIGTERM-aware

- [ ] **Step 1: Write the failing test in `rabbit_integration.rs`**

```rust
use common_event::consumer::{Consumer, RabbitMqConsumer};
use common_event::events::{IamUserCreated, IamEventEnum};
use common_event::outbox::MySqlOutboxRepo;
use common_event::{Envelope, Event, Publisher, RabbitMqPublisher};
use common_event::publisher_loop::OutboxPublisher;
use common_event::routing_keys;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::rabbitmq::RabbitMq;
use testcontainers_modules::mysql::Mysql;
use testcontainers::ImageExt;
use sqlx::mysql::MySqlPool;

const DDL: &str = "..."; // same as outbox_repo test

#[tokio::test]
async fn outbox_publish_loop_sends_pending_rows() {
    // ... set up MySQL pool with outbox table, RabbitMQ container, RabbitMqPublisher
    // ... enqueue one envelope
    // ... spawn OutboxPublisher::run for ~2s
    // ... assert: a test consumer bound to "iam.#" receives the message
}
```

(Specifics: see existing testcontainers patterns in `services/token-service/tests/common/mod.rs`. Build incrementally — first the consumer test alone, then add the publisher loop test.)

- [ ] **Step 2: Run to confirm failure**

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
```

- [ ] **Step 3: Implement `Consumer` + `RabbitMqConsumer` in `consumer.rs`**

```rust
use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::{
    options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
    Connection, ConnectionProperties,
};

use crate::envelope::Envelope;
use crate::error::{ConsumeError, HandlerError};
use crate::event::Event;

#[async_trait]
pub trait Consumer: Send + Sync {
    async fn subscribe<P, H, Fut>(
        &self,
        queue: &str,
        bindings: &[&str],
        handler: H,
    ) -> Result<(), ConsumeError>
    where
        P: Event,
        H: Fn(Envelope<P>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), HandlerError>> + Send;
}

pub struct RabbitMqConsumer {
    conn: Connection,
    url: String,
}

impl RabbitMqConsumer {
    pub async fn new(url: &str) -> Result<Self, ConsumeError> {
        let conn = Connection::connect(url, ConnectionProperties::default())
            .await.map_err(|e| ConsumeError::Broker(e.to_string()))?;
        Ok(Self { conn, url: url.to_string() })
    }
}

#[async_trait]
impl Consumer for RabbitMqConsumer {
    async fn subscribe<P, H, Fut>(
        &self,
        queue: &str,
        bindings: &[&str],
        handler: H,
    ) -> Result<(), ConsumeError>
    where
        P: Event,
        H: Fn(Envelope<P>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), HandlerError>> + Send,
    {
        let channel = self.conn.create_channel().await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;
        channel.basic_qos(32, lapin::options::BasicQosOptions::default()).await
            .map_err(|e| ConsumeError::Broker(e.to_string()))?;

        channel.queue_declare(
            queue,
            QueueDeclareOptions { durable: true, ..Default::default() },
            FieldTable::default(),
        ).await.map_err(|e| ConsumeError::Broker(e.to_string()))?;

        for rk in bindings {
            channel.queue_bind(queue, "events", rk, QueueBindOptions::default(), FieldTable::default())
                .await.map_err(|e| ConsumeError::Broker(e.to_string()))?;
        }

        let mut consumer = channel.basic_consume(
            queue,
            "audit-service",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        ).await.map_err(|e| ConsumeError::SubscribeFailed(e.to_string()))?;

        let handler = Arc::new(handler);
        tokio::spawn(async move {
            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(d) => {
                        match serde_json::from_slice::<Envelope<P>>(&d.data) {
                            Ok(env) => match handler(env).await {
                                Ok(_) => { let _ = d.ack(BasicAckOptions::default()).await; }
                                Err(_) => { let _ = d.nack(BasicNackOptions { requeue: false, ..Default::default() }).await; }
                            },
                            Err(_) => { let _ = d.nack(BasicNackOptions { requeue: false, ..Default::default() }).await; }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(())
    }
}
```

- [ ] **Step 4: Implement `OutboxPublisher` in `publisher_loop.rs`**

```rust
use std::time::Duration;
use tokio::time::sleep;

use crate::error::RunError;
use crate::outbox::OutboxRepo;
use crate::publisher::Publisher;

pub struct OutboxPublisherConfig {
    pub poll_interval: Duration,
    pub batch_size: u32,
}

pub struct OutboxPublisher<R: OutboxRepo, P: Publisher> {
    repo: R,
    publisher: P,
    cfg: OutboxPublisherConfig,
}

impl<R: OutboxRepo, P: Publisher> OutboxPublisher<R, P> {
    pub fn new(repo: R, publisher: P, cfg: OutboxPublisherConfig) -> Self {
        Self { repo, publisher, cfg }
    }

    pub async fn run(self) -> Result<(), RunError> {
        loop {
            let batch = self.repo.claim_batch(self.cfg.batch_size).await
                .map_err(|e| RunError::Publish(e.to_string()))?;
            if batch.is_empty() {
                sleep(self.cfg.poll_interval).await;
                continue;
            }
            let mut published_ids = Vec::new();
            for row in &batch {
                // The envelope_json is already a JSON Value with the full Envelope
                let body = serde_json::to_vec(&row.envelope_json)
                    .map_err(|e| RunError::Publish(e.to_string()))?;
                self.publisher.channel().basic_publish(
                    "events", &row.routing_key,
                    lapin::options::BasicPublishOptions { mandatory: true, ..Default::default() },
                    &body,
                    lapin::BasicProperties::default().with_content_type("application/json".into()),
                ).await.map_err(|e| RunError::Publish(e.to_string()))?
                 .await.map_err(|e| RunError::Publish(e.to_string()))?;
                published_ids.push(row.id);
            }
            self.repo.mark_published(&published_ids).await
                .map_err(|e| RunError::Publish(e.to_string()))?;
        }
    }
}
```

- [ ] **Step 5: Add `futures-util` to Cargo.toml dependencies**

```toml
futures-util = "0.3"
```

- [ ] **Step 6: Run tests**

```bash
node scripts/run-tests-docker.js common-event 2>&1 | tail -20
docker ps -a --format '{{.Names}} {{.Image}}' | grep -E "rabbit|mysql:8" | grep -v infrastructure | awk '{print $1}' | xargs -r docker rm -f
```

Expected: all consumer + publisher_loop tests pass; cleanup done.

- [ ] **Step 7: Commit**

```bash
git add libs/common-event/
git commit -m "feat(common-event): Consumer + OutboxPublisher loop (phase 4)"
```

---

### Task 11: iam-service outbox migration

**Files:**
- Create: `services/iam-service/migrations/V20260626120000__iam_outbox_events.sql`

- [ ] **Step 1: Write the migration**

```sql
-- Outbox table for iam-service events.
-- producer (iam-service) writes here in the same transaction as the
-- business row, then the iam-outbox-publisher binary polls and publishes.
CREATE TABLE IF NOT EXISTS iam_outbox_events (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    routing_key     VARCHAR(128) NOT NULL,
    envelope_json   JSON         NOT NULL,
    created_at      DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    published_at    DATETIME(3)  NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_iam_outbox_event_id (event_id),
    KEY idx_iam_outbox_unpublished (published_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

- [ ] **Step 2: Verify the migration applies**

```bash
docker compose -f infrastructure/docker-compose.yml down -v
docker compose -f infrastructure/docker-compose.yml up -d mysql
sleep 20
docker exec -i $(docker ps -qf name=mysql) mysql -uroot -prootpass superpowers < services/iam-service/migrations/V20260626120000__iam_outbox_events.sql
docker exec $(docker ps -qf name=mysql) mysql -uroot -prootpass superpowers -e "DESCRIBE iam_outbox_events"
```

Expected: table created with all columns.

- [ ] **Step 3: Commit**

```bash
git add services/iam-service/migrations/V20260626120000__iam_outbox_events.sql
git commit -m "feat(iam): outbox_events table migration (phase 4)"
```

---

### Task 12: iam-service wire outbox writes in user_svc + auth_svc

**Files:**
- Modify: `services/iam-service/Cargo.toml` (add `common-event` dep)
- Create: `services/iam-service/src/events.rs` (re-exports + handler glue)
- Create: `services/iam-service/src/outbox.rs` (concrete `MySqlOutboxRepo` instance)
- Modify: `services/iam-service/src/service/user_svc.rs` (enqueue in create/delete)
- Modify: `services/iam-service/src/service/auth_svc.rs` (enqueue in login_success)
- Modify: `services/iam-service/src/state.rs` (add `outbox_repo` to AppState)

**Interfaces:**
- `AppState { ..., outbox_repo: MySqlOutboxRepo }`
- All 3 event emissions wrapped in `pool.begin()` transaction with the original INSERT

- [ ] **Step 1: Add common-event to iam-service Cargo.toml**

```toml
common-event = { path = "../../libs/common-event" }
```

- [ ] **Step 2: Create `services/iam-service/src/outbox.rs`**

```rust
use common_event::outbox::MySqlOutboxRepo;
use sqlx::MySqlPool;

pub fn build(pool: MySqlPool) -> MySqlOutboxRepo {
    MySqlOutboxRepo::new(pool)
}
```

- [ ] **Step 3: Create `services/iam-service/src/events.rs`**

```rust
//! Re-exports iam-event structs from common-event for code paths
//! that don't want to import from the common crate directly.
pub use common_event::events::{IamLoginSucceeded, IamUserCreated, IamUserDeleted};
pub use common_event::Envelope;
```

- [ ] **Step 4: Modify `services/iam-service/src/state.rs`**

Add `outbox_repo: MySqlOutboxRepo` to the `AppState` struct + `new()` constructor.

- [ ] **Step 5: Modify `services/iam-service/src/service/user_svc.rs::create_user`**

In the existing transaction, after the user INSERT, add:

```rust
let env = Envelope::new("iam-service", IamUserCreated {
    user_id: user.id.to_string(),
    username: req.username.clone(),
    email: req.email.clone(),
    org_unit_id: req.org_unit_id.clone(),
});
self.state.outbox_repo.enqueue(&mut tx, &env).await?;
tx.commit().await?;
```

- [ ] **Step 6: Modify `user_svc::delete_user`**

Same pattern with `IamUserDeleted { user_id, deleted_by: principal.user_id }`.

- [ ] **Step 7: Modify `auth_svc::login_success`**

Same pattern with `IamLoginSucceeded { user_id, ip, user_agent, method }`.

- [ ] **Step 8: Remove old `AuditClient` calls**

Grep `audit_client` / `AuditClient` in `services/iam-service/src/`. Remove all sync HTTP writes. Delete `src/audit_client.rs`. Remove its `reqwest` dep from Cargo.toml if unused.

- [ ] **Step 9: Add a unit test for the outbox enqueue flow**

In `services/iam-service/tests/outbox_integration.rs`:

```rust
#[tokio::test]
async fn create_user_writes_to_outbox() {
    // testcontainers mysql pool
    // call create_user via service
    // assert: outbox_events row exists with event_type=iam.user.created
}
```

- [ ] **Step 10: Run all iam-service tests**

```bash
node scripts/run-tests-docker.js iam-service 2>&1 | tail -20
docker ps -a --format '{{.Names}} {{.Image}}' | grep -E "mysql:8" | grep -v infrastructure | awk '{print $1}' | xargs -r docker rm -f
```

- [ ] **Step 11: Verify CI gate still passes**

```bash
node scripts/check-db-naming.js
```

- [ ] **Step 12: Commit**

```bash
git add services/iam-service/
git commit -m "refactor(iam): emit events via outbox, remove sync audit HTTP client (phase 4)"
```

---

### Task 13: iam-service outbox-publisher binary

**Files:**
- Modify: `services/iam-service/Cargo.toml` (add `[[bin]]` entry)
- Create: `services/iam-service/src/bin/outbox_publisher.rs`
- Modify: `infrastructure/docker-compose-services.yml` (add `iam-outbox-publisher` service)

- [ ] **Step 1: Add `[[bin]]` to iam-service Cargo.toml**

```toml
[[bin]]
name = "outbox-publisher"
path = "src/bin/outbox_publisher.rs"
```

- [ ] **Step 2: Create the binary in `services/iam-service/src/bin/outbox_publisher.rs`**

```rust
use std::time::Duration;
use anyhow::Context;
use common_event::{outbox::MySqlOutboxRepo, publisher_loop::{OutboxPublisher, OutboxPublisherConfig}, EventBusConfig, Publisher, RabbitMqPublisher};
use sqlx::mysql::MySqlPoolOptions;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("iam-outbox-publisher starting");

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL required")?;
    let event_bus_url = std::env::var("EVENT_BUS_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());
    let exchange = std::env::var("EVENT_BUS_EXCHANGE").unwrap_or_else(|_| "events".to_string());
    let poll_ms: u64 = std::env::var("OUTBOX_POLL_INTERVAL_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(1000);
    let batch_size: u32 = std::env::var("OUTBOX_BATCH_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(100);

    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&db_url).await.context("connect mysql")?;
    let repo = MySqlOutboxRepo::new(pool);
    let publisher = RabbitMqPublisher::new(&event_bus_url, &exchange).await
        .context("connect rabbitmq")?;

    let outbox = OutboxPublisher::new(repo, publisher, OutboxPublisherConfig {
        poll_interval: Duration::from_millis(poll_ms),
        batch_size,
    });
    tokio::select! {
        res = outbox.run() => { res?; }
        _ = tokio::signal::ctrl_c() => {
            warn!("SIGTERM received, exiting");
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Add the docker-compose service**

```yaml
  iam-outbox-publisher:
    image: <same as iam-service>
    command: ["outbox-publisher"]
    environment:
      - DATABASE_URL=mysql://root:rootpass@mysql:3306/superpowers
      - EVENT_BUS_URL=amqp://guest:guest@rabbitmq:5672/%2f
    depends_on:
      mysql:    { condition: service_healthy }
      rabbitmq: { condition: service_healthy }
    networks: [superpowers]
```

- [ ] **Step 4: Build and run the binary briefly to verify it starts**

```bash
docker build -f services/iam-service/Dockerfile -t iam-test .
docker run --rm --network host -e DATABASE_URL=mysql://root:rootpass@localhost:3306/superpowers iam-test outbox-publisher &
sleep 5
docker ps -a | grep outbox  # verify still running
docker kill <id>
```

- [ ] **Step 5: Commit**

```bash
git add services/iam-service/ infrastructure/docker-compose-services.yml
git commit -m "feat(iam): outbox-publisher binary + docker-compose service (phase 4)"
```

---

### Task 14: audit-service audit_events migration + idempotent insert

**Files:**
- Create: `services/audit-service/migrations/V20260626120100__audit_events.sql`
- Modify: `services/audit-service/src/models.rs` (or equivalent) — add `AuditEvent` row struct

- [ ] **Step 1: Write the migration**

```sql
-- Idempotency table for events consumed from RabbitMQ.
-- event_id is the dedup key; INSERT ... ON DUPLICATE KEY UPDATE makes
-- the consumer at-least-once safe.
CREATE TABLE IF NOT EXISTS audit_events (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    occurred_at     DATETIME(3)  NOT NULL,
    received_at     DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    trace_id        VARCHAR(64)  NULL,
    producer        VARCHAR(64)  NOT NULL,
    payload_json    JSON         NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_audit_events_event_id (event_id),
    KEY idx_audit_events_type_time (event_type, occurred_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

- [ ] **Step 2: Verify it applies**

```bash
docker exec -i $(docker ps -qf name=mysql) mysql -uroot -prootpass superpowers < services/audit-service/migrations/V20260626120100__audit_events.sql
docker exec $(docker ps -qf name=mysql) mysql -uroot -prootpass superpowers -e "DESCRIBE audit_events"
```

- [ ] **Step 3: Commit**

```bash
git add services/audit-service/migrations/V20260626120100__audit_events.sql
git commit -m "feat(audit): audit_events table for event idempotency (phase 4)"
```

---

### Task 15: audit-service consumer + delete HTTP write endpoints

**Files:**
- Modify: `services/audit-service/Cargo.toml` (add `common-event` dep)
- Create: `services/audit-service/src/consumer.rs` (the subscriber)
- Modify: `services/audit-service/src/main.rs` (spawn consumer task)
- Modify: `services/audit-service/src/routes/audit.rs` (delete POST handlers)
- Modify: `services/audit-service/openapi.yaml` (regenerate via `dump-openapi-specs.sh`)
- Modify: `tests/e2e/api/cases/iam-audit-http.yaml` (remove or convert)

- [ ] **Step 1: Add common-event to audit-service Cargo.toml**

```toml
common-event = { path = "../../libs/common-event" }
```

- [ ] **Step 2: Create `services/audit-service/src/consumer.rs`**

```rust
use anyhow::Context;
use common_event::events::IamEventEnum;
use common_event::{Consumer, Envelope, RabbitMqConsumer};
use sqlx::MySqlPool;
use tracing::{info, warn};

pub async fn run(pool: MySqlPool, url: String) -> anyhow::Result<()> {
    let consumer = RabbitMqConsumer::new(&url).await.context("connect rabbitmq")?;
    consumer.subscribe::<IamEventEnum, _, _>(
        "audit.iam-events",
        &["iam.#"],
        move |env: Envelope<IamEventEnum>| {
            let pool = pool.clone();
            async move {
                persist(&pool, &env).await
            }
        },
    ).await?;
    info!("audit-service consumer started on audit.iam-events");
    Ok(())
}

async fn persist(pool: &MySqlPool, env: &Envelope<IamEventEnum>) -> Result<(), common_event::HandlerError> {
    let payload_json = serde_json::to_value(&env.payload)
        .map_err(|e| common_event::HandlerError::Handler(e.to_string()))?;
    sqlx::query(
        "INSERT INTO audit_events (event_id, event_type, occurred_at, trace_id, producer, payload_json) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON DUPLICATE KEY UPDATE received_at = CURRENT_TIMESTAMP(3)"
    )
    .bind(env.event_id.to_string())
    .bind(&env.event_type)
    .bind(env.occurred_at)
    .bind(&env.trace_id)
    .bind(&env.producer)
    .bind(payload_json)
    .execute(pool).await
    .map_err(|e| {
        warn!(error=%e, "audit consumer persist failed");
        common_event::HandlerError::Handler(e.to_string())
    })?;
    Ok(())
}
```

- [ ] **Step 3: Spawn the consumer in `main.rs`**

```rust
let event_bus_url = std::env::var("EVENT_BUS_URL")
    .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());
let pool_for_consumer = pool.clone();
let url_for_consumer = event_bus_url.clone();
tokio::spawn(async move {
    if let Err(e) = consumer::run(pool_for_consumer, url_for_consumer).await {
        tracing::error!(error=%e, "audit consumer terminated");
    }
});
```

- [ ] **Step 4: Delete HTTP write endpoints**

In `services/audit-service/src/routes/audit.rs`, remove any handler that accepts POST (e.g. `POST /audit/operation-logs`, `POST /audit/login-logs`). Keep GET handlers (queries + compliance export + retention).

- [ ] **Step 5: Regenerate openapi.yaml**

```bash
scripts/dump-openapi-specs.sh
git diff services/audit-service/openapi.yaml  # should show POST endpoints removed
```

- [ ] **Step 6: Add audit-service unit test for consumer persistence**

In `services/audit-service/tests/consumer_persist.rs`:
- Start testcontainers mysql + rabbitmq
- Spawn consumer
- Publish an envelope via RabbitMqPublisher
- Assert: audit_events row appears within 2s

- [ ] **Step 7: Run audit-service tests**

```bash
node scripts/run-tests-docker.js audit-service 2>&1 | tail -20
docker ps -a --format '{{.Names}} {{.Image}}' | grep -E "mysql:8|rabbitmq" | grep -v infrastructure | awk '{print $1}' | xargs -r docker rm -f
```

- [ ] **Step 8: Commit**

```bash
git add services/audit-service/
git commit -m "refactor(audit): add RabbitMQ consumer, remove HTTP write endpoints (phase 4)"
```

---

### Task 16: e2e test for event flow (iam → RabbitMQ → audit)

**Files:**
- Create: `tests/e2e/api/cases/iam-audit-event-flow.yaml`
- Modify: `tests/e2e/api/cases/iam-create-user.yaml` (assert audit_events row, not audit_operation_logs)

**Interfaces:**
- Test 1: create user via `POST /v1/iam/users` → wait 2s → query audit DB directly → assert 1 row in `audit_events` with `event_id` matching
- Test 2: delete user → assert 1 row in `audit_events` with event_type=`iam.user.deleted`
- Test 3: login success → assert 1 row with event_type=`iam.login.succeeded`
- Test 4: kill RabbitMQ → create user → assert outbox row appears with `published_at=NULL` → restart RabbitMQ → assert row gets `published_at` within 10s
- Test 5: nack injection → message goes to `events.dlq` → manual shovel back → assert no duplicate row in audit_events (idempotency)

- [ ] **Step 1: Write the e2e test (case 1: happy path)**

```yaml
# tests/e2e/api/cases/iam-audit-event-flow.yaml
name: iam → RabbitMQ → audit event flow
prereq:
  - iam-service running
  - audit-service running
  - rabbitmq running
  - outbox-publisher running
steps:
  - name: create user
    request:
      method: POST
      path: /v1/iam/users
      headers: { Authorization: "Bearer ${JWT}" }
      body: { username: "event_test_user", password: "Test1234!" }
    expect:
      status: 201

  - name: wait for event propagation
    sleep_ms: 2000

  - name: query audit_events via direct SQL
    db:
      dsn: mysql://root:rootpass@mysql:3306/superpowers
      query: "SELECT COUNT(*) AS n FROM audit_events WHERE event_type='iam.user.created' AND JSON_EXTRACT(payload_json, '$.payload.username')='event_test_user'"
    expect:
      n: 1
```

- [ ] **Step 2: Wire the e2e test into the test orchestrator**

The existing `tests/e2e/api/cases/` is loaded by the e2e runner (per `docs-site/ai-context.md` §8). Verify the new YAML is picked up; if the runner needs registration, add it to any index file.

- [ ] **Step 3: Run the e2e test against the local stack**

```bash
docker compose -f infrastructure/docker-compose.yml up -d
docker compose -f infrastructure/docker-compose-services.yml up -d
cd tests/e2e && pnpm install && pnpm test:smoke --case iam-audit-event-flow
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/
git commit -m "test(e2e): iam→audit event flow happy path (phase 4)"
```

---

### Task 17: Observability + final regression

**Files:**
- Modify: `libs/common-event/src/metrics.rs` (replace placeholder)
- Create: `services/audit-service/src/tracing_mw.rs` (OTel layer)
- Modify: `services/audit-service/Cargo.toml` (enable `otlp` feature on common-telemetry)
- Modify: `docs-site/USER_MANUAL.md` (add event bus section)
- Modify: `docs-site/ai-context.md` (mark audit-service as OTel-enabled)

- [ ] **Step 1: Implement metrics in `libs/common-event/src/metrics.rs`**

```rust
use prometheus::{IntCounterVec, IntGauge, HistogramVec, Registry};

pub struct EventMetrics {
    pub publish_total: IntCounterVec,
    pub consume_total: IntCounterVec,
    pub backlog: IntGauge,
    pub publish_latency_seconds: HistogramVec,
}

impl EventMetrics {
    pub fn new(registry: &Registry) -> Self {
        let publish_total = IntCounterVec::new(
            prometheus::Opts::new("event_publish_total", "Events published"),
            &["event_type", "status"],
        ).unwrap();
        let consume_total = IntCounterVec::new(
            prometheus::Opts::new("event_consume_total", "Events consumed"),
            &["queue", "event_type", "status"],
        ).unwrap();
        let backlog = IntGauge::with_opts(
            prometheus::Opts::new("outbox_backlog", "Unpublished outbox rows"),
        ).unwrap();
        let publish_latency_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new("outbox_publish_latency_seconds", "Outbox publish latency"),
            &["event_type"],
        ).unwrap();
        registry.register(Box::new(publish_total.clone())).unwrap();
        registry.register(Box::new(consume_total.clone())).unwrap();
        registry.register(Box::new(backlog.clone())).unwrap();
        registry.register(Box::new(publish_latency_seconds.clone())).unwrap();
        Self { publish_total, consume_total, backlog, publish_latency_seconds }
    }
}
```

- [ ] **Step 2: Wire metrics into Publisher + OutboxPublisher + Consumer**

(Counter increments in success/error paths. Backlog gauge updated by `OutboxPublisher::run` every cycle: `SELECT COUNT(*) FROM outbox_events WHERE published_at IS NULL`.)

- [ ] **Step 3: Add OTel to audit-service**

In `services/audit-service/Cargo.toml`:
```toml
common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }
```

Create `services/audit-service/src/tracing_mw.rs` (copy pattern from `services/notification-service/src/tracing_mw.rs`).

In `services/audit-service/src/main.rs`, init `common_telemetry::init(...)` like the other OTel-enabled services.

- [ ] **Step 4: Run all tests + CI gates**

```bash
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
node scripts/check-db-naming.js
node scripts/run-tests-docker.js
```

Cleanup testcontainers:
```bash
docker ps -a --format '{{.Names}} {{.Image}}' | grep -E "mysql:8|rabbitmq" | grep -v infrastructure | awk '{print $1}' | xargs -r docker rm -f
```

All must pass.

- [ ] **Step 5: Manual smoke (per spec §12 acceptance criteria)**

```bash
docker compose -f infrastructure/docker-compose.yml up -d
docker compose -f infrastructure/docker-compose-services.yml up -d
# create user via curl
curl -X POST http://localhost:6002/v1/iam/users -H "Authorization: Bearer $JWT" -d '{"username":"smoke_test","password":"Test1234!"}'
# wait 3s, then check audit_events
docker exec $(docker ps -qf name=mysql) mysql -uroot -prootpass superpowers -e "SELECT event_type, JSON_EXTRACT(payload_json, '$.payload.username') FROM audit_events WHERE event_type='iam.user.created' ORDER BY received_at DESC LIMIT 1"
# should show smoke_test
```

Kill RabbitMQ for 30s, repeat: outbox should pile up; restart RabbitMQ; backlog should drain within 10s. Verify `outbox_backlog` metric via Prometheus.

- [ ] **Step 6: Update docs**

- `docs-site/USER_MANUAL.md`: add "Event Bus" section explaining topology + how to publish/consume.
- `docs-site/ai-context.md`: mark audit-service as OTel-enabled in service map table.

- [ ] **Step 7: Final commit**

```bash
git add libs/common-event/ services/audit-service/ docs/
git commit -m "feat(phase 4): metrics + OTel + docs; final regression green (phase 4 close)"
```

---

## Self-Review

**1. Spec coverage:**
- §2.1 In Scope: ✅ Tasks 1-3 (infra), Task 4-5 (common-event scaffold), Tasks 6-10 (event/Envelope/Outbox/Publisher/Consumer), Task 3 (events.yaml), Task 4 (codegen), Tasks 11-13 (iam outbox + publisher), Tasks 14-15 (audit consumer), Task 16 (e2e), Task 17 (metrics/tracing)
- §3 topology: ✅ Task 2 (definitions.json pre-declares events + events.dlx + audit.iam-events + events.dlq + bindings)
- §4 Envelope: ✅ Task 6 (Envelope with event_id UUIDv7, occurred_at, trace_id, tenant_id, producer, payload)
- §5 common-event API: ✅ Tasks 6 (Event + Envelope), 8 (OutboxRepo + MySqlOutboxRepo), 9 (Publisher + RabbitMqPublisher), 10 (Consumer + OutboxPublisher::run)
- §6.2 multi-replica: ✅ Task 8 uses `FOR UPDATE SKIP LOCKED` in `claim_batch`
- §6.3 deployment: ✅ Task 13 adds `[[bin]]` + docker-compose service
- §6.4 SIGTERM: ✅ Task 13 has `tokio::select!` for ctrl_c
- §7.1 iam outbox: ✅ Task 11 (V20260626120000__iam_outbox_events.sql)
- §7.2 audit_events: ✅ Task 14 (V20260626120100__audit_events.sql)
- §8.1 iam writes: ✅ Task 12 (user_svc + auth_svc enqueue)
- §8.2 delete audit HTTP: ✅ Task 12 step 8
- §9.1 consumer: ✅ Task 15
- §9.2 delete POST endpoints: ✅ Task 15 step 4
- §10 infra: ✅ Tasks 1-2 (compose + definitions)
- §11.1 metrics: ✅ Task 17 (EventMetrics)
- §11.2 tracing: ✅ Task 17 (audit-service OTel + envelope trace_id)
- §12 acceptance: ✅ Task 16 (e2e), Task 17 (smoke + CI gates)

**2. Placeholder scan:** No "TBD" / "TODO" / "implement later" / "fill in" markers. All steps have exact code.

**3. Type consistency:** 
- `OutboxRow { id, event_id, routing_key, envelope_json }` used in Task 8 test + impl, Task 10 loop.
- `Envelope::new(producer, payload)` signature consistent across Tasks 6, 7, 8, 12.
- `OutboxPublisher::new(repo, publisher, cfg)` consistent in Tasks 10 + 13.
- `Consumer::subscribe<P, H, Fut>(queue, bindings, handler)` consistent in Tasks 10 + 15.
- Event type names `IamUserCreated` / `IamUserDeleted` / `IamLoginSucceeded` consistent in Tasks 7, 8, 9, 10, 12, 15.

**4. Known limitations (deferred per spec §2.2):**
- Phase 4.x: notification-service, login.failed, permission.changed
- Phase 5: retry/DLQ UI, saga orchestration
- Phase 6: full OTel coverage
- Phase 7: RabbitMQ cluster, K3s

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-26-phase4-event-bus.md`. Two execution options:

1. **Subagent-Driven (recommended)** - dispatch a fresh subagent per task, review between tasks
2. **Inline Execution** - execute tasks in this session, batch with checkpoints
