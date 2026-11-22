# MT-01 — Template Hardening Report

**Status:** implementation slice completed; CI/runtime verification pending  
**Branch:** `mt-01-template-hardening`  
**Scope:** new-service scaffold, Cargo registry configuration, IAM process lifecycle

## Changes

### 1. New-service scaffold

Updated `.agents/skills/coder-new-service/SKILL.md`:

- Removed the private Kellnr registry, its internal address, and token/build-arg instructions. The platform `digua_*` crates are referenced through crates.io versions.
- Updated the example crate versions to the repository's currently declared published baseline (`1.6.0`). Before generating a service, verify the selected crate version on crates.io and keep all digua crates on a compatible release set.
- Removed the hard-coded `root:rootpass` database URL. Database-backed services must receive `DATABASE_URL`; no-DB services must omit SQLx and database crates.
- Changed environment parsing to a fallible `try_from_env()`; invalid port/bind values now fail startup rather than silently falling back.
- Updated the MySQL state example to create its pool from explicit configuration and made readiness probe the database.
- Added distinct liveness/readiness endpoints while retaining `/health` as a compatibility route for existing service checks and Consul probes.
- Changed Prometheus metrics output to the Prometheus text content type and explicit encoding error handling.
- Changed the public error response to avoid returning raw database/internal error strings.
- Updated the Docker example to use the public Rust image, copy the workspace directories required by Cargo, and avoid private registry credentials.
- Updated validation commands to use the repository's Cargo/test gates without Kellnr variables.

The route/state example is intentionally a **MySQL-backed variant**. PostgreSQL, SQLite, and no-DB variants must be generated with their own pool/state code and matching Cargo features; do not copy the MySQL-specific `MySqlPool` into those variants.

### 2. Cargo registry configuration

Updated `.cargo/config.toml` to remove the obsolete private Kellnr source. The existing crates.io mirror remains as a download acceleration layer; it is not a separate private crate registry.

### 3. IAM lifecycle and Swagger flag

Updated `services/iam-service/src/main.rs`:

- Corrected `SWAGGER_ENABLED` semantics: Swagger is disabled by default and enabled only by explicit `true` / `1`.
- Replaced the shutdown task's `std::process::exit(0)` with Axum graceful shutdown. The server now drains in-flight requests, stops the JWKS refresh loop, and deregisters from Consul after the HTTP server has stopped.

Swagger remains opt-in because the source contains a known `utoipa-swagger-ui` SIGSEGV issue. This change corrects the flag semantics; it does not claim to fix that upstream/runtime defect.

## Verification

- [x] Reviewed current `main` contents before editing.
- [x] Created an isolated branch from `main`.
- [x] Re-read the edited scaffold and removed private Kellnr addresses/token instructions from it.
- [ ] `cargo fmt --check`
- [ ] `cargo check --workspace`
- [ ] `cargo test --workspace`
- [ ] `node scripts/check-service-standard.js`
- [ ] Docker build of a generated no-DB service and a generated MySQL service.
- [ ] Runtime SIGTERM test confirming request drain and Consul deregistration.
- [ ] GitLab pipeline verification. The repository's current workflow has an explicit `LMMS_CI_ENABLED=true` gate, so a normal push does not start the pipeline unless that variable is supplied.

**No local Cargo, Docker, or GitLab CI test is claimed as passed in this report.** The edits were made through GitHub's repository API; compile/runtime verification remains required.

## Follow-up items (not silently considered fixed)

- MT-02: audit public crate dependency direction, feature defaults, and published API compatibility. In particular, review `digua_common_error`'s Axum/SQLx/reqwest dependencies and the `digua_common_db` advertised dialect support.
- MT-03: audit JWT propagation/rotation, tenant isolation, Redis failure modes, outbox idempotency/replay, timeout/retry policy, and service health semantics across all services.
- MT-04: add generated-service fixture projects (no-DB/MySQL/PostgreSQL) and make them blocking CI checks.
- The current `.gitlab-ci.yml` CI pause switch is intentionally unchanged; decide separately whether to remove it or keep manual pipeline activation.
