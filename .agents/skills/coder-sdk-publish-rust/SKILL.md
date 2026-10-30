---
name: coder-sdk-publish-rust
description: |
  Publish Rust crates to the private kellnr registry:
  - libs/* (17 shared library crates, e.g. digua_common_error, digua_jwt_ctx)
  - sdk/rust/* (1 base + 6 service SDK crates, e.g. digua_audit_sdk)
  Use when the user says "publish rust crates", "publish rust libs",
  "publish rust sdk", "sdk:publish:libs", "sdk:publish:rust",
  "发布 rust crate / rust libs / rust sdk", "cargo publish kellnr",
  "上传 Rust 包", or any equivalent intent. Cross-platform; reads
  CARGO_TARGET_DIR with sensible Windows / Linux / macOS defaults.
---

# coder-sdk-publish-rust Skill

Publish Rust crates to the private **kellnr** registry
(`http://192.168.2.12:16444`). Two independent pipelines:

| Pipeline | Crates | Command | Script |
| --- | --- | --- | --- |
| **libs** (shared platform libraries) | 17 crates in `libs/*` | `pnpm run sdk:publish:libs` | `coder release publish rust --targets libs` |
| **Rust SDK** (per-service client SDKs) | 7 crates in `sdk/rust/*` (1 base + 6 service) | `pnpm run sdk:publish:rust` | `coder release publish rust` |

Both pipelines:

- Publish **in topological order** (libs use a 3-layer DAG; SDK uses
  base-first, then services)
- Temporarily rewrite `path = "../xxx"` deps as `version = "<v>", registry = "kellnr"`
  per-crate, then restore the original `Cargo.toml` after each `cargo publish`
- Restore tombstones (`.bak` files) on both success and failure paths
- Support `--dry-run` for end-to-end validation without actually publishing

## When to use which

| User intent | Pipeline |
| --- | --- |
| "Bump libs to 0.3.0" / "publish digua_common_error 0.4.0" | `sdk:publish:libs` |
| "Publish iam-service-sdk 0.5.0" / "publish all Rust SDKs" | `sdk:publish:rust` |
| "Publish both" | run **both** in sequence, libs **first** (SDK depends on libs through consumer code, not registry itself, but the SDK base crate `digua-http-client` is independent) |

> The two pipelines are **independent**. `digua-http-client` (SDK base)
> does not depend on `libs/*`. The ordering between `sdk:publish:libs`
> and `sdk:publish:rust` does not matter; pick by user intent.

## Pre-flight (mandatory before either pipeline)

```bash
bash .agents/skills/coder-sdk-publish-rust/scripts/preflight.sh \
  <version> [libs|sdk|all]
```

What the preflight checks:

1. `cargo` and `node` are on PATH
2. `CARGO_REGISTRIES_KELLNR_TOKEN` env var is set (the script prints the
   `cargo login` instructions if it isn't)
3. `http://192.168.2.12:16444/` is reachable
4. For `libs`: all 17 expected crate directories exist under `libs/`
   and every `Cargo.toml` has `description` and `license` fields
5. For `sdk`: all 7 expected crate directories exist under `sdk/rust/`
   and every `Cargo.toml` has `description` and `license` fields
6. `cargo metadata --format-version 1` succeeds (no broken path deps)
7. Working tree is clean in the relevant directory
8. Reports a sensible default `CARGO_TARGET_DIR` per-OS

**Do not proceed if any check fails.** Each script hard-`exit 1`es on
the first violation.

## Cross-platform CARGO_TARGET_DIR

Publishing 24+ crates can fill the default `target/` directory and
make Docker's `coder-cargo-target` volume fight for ownership. The
project AGENTS.md mandates an override:

```bash
# Linux / macOS
export CARGO_TARGET_DIR=/tmp/libs-target
pnpm run sdk:publish:libs 1.2.0

# Windows PowerShell
$env:CARGO_TARGET_DIR = "$env:TEMP\libs-target"
pnpm run sdk:publish:libs 1.2.0
```

Do NOT `sudo chown` the repo's `target/` — it gets clobbered by Docker
anyway. Always use `CARGO_TARGET_DIR`.

## Release Flow — libs (`sdk:publish:libs`)

### Step 1 — Confirm intent

Show user:

- Topological order (Layer 0 → 1 → 2): see `coder release publish rust --targets libs`
  `PUBLISH_ORDER` array
- Number of crates: **17**
- Target version + registry (default `kellnr`)
- Whether `--dry-run` will be used

### Step 2 — Pre-flight

```bash
bash .agents/skills/coder-sdk-publish-rust/scripts/preflight.sh <version> libs
```

### Step 3 — Bump versions (optional — script reads existing version)

The script does NOT auto-bump. Either:

```bash
# Manual bump all 17 to a single version (do this before publishing)
for d in libs/common-error libs/common-health libs/common-jwt \
         libs/common-ratelimit libs/common-telemetry libs/common-cache \
         libs/common-config libs/common-crypto libs/common-event \
         libs/common-http libs/common-log libs/common-validator \
         libs/common-telemetry-axum libs/jwt-ctx libs/common-db \
         libs/authz libs/common; do
  sed -i.bak -E "0,/^version = \"[^\"]+\"/s//version = \"${VERSION}\"/" "${d}/Cargo.toml"
done
# Or use cargo set-version (cargo-edit must be installed)
cargo set-version --workspace "${VERSION}" --manifest-path libs/Cargo.toml
```

…then verify with `git diff libs/`.

### Step 4 — Dry-run (mandatory first time)

```bash
CARGO_TARGET_DIR=/tmp/libs-target \
  pnpm run sdk:publish:libs 1.2.0 --dry-run
```

Inspect output for any crate that fails dry-run. Common causes: missing
`description`, missing `license`, broken path dep that wasn't rewritten
correctly, or a non-deterministic `Cargo.lock`.

### Step 5 — Real publish

```bash
CARGO_TARGET_DIR=/tmp/libs-target \
  pnpm run sdk:publish:libs 1.2.0
```

Re-running is **idempotent** — crates already at `<version>` on the
registry will be skipped by `cargo publish` (registry 400 error, not
fatal at script level).

### Step 6 — Tag

```bash
git tag v<version>-libs
git push origin main --follow-tags
```

## Release Flow — Rust SDK (`sdk:publish:rust`)

Same shape, but:

- Crate set is **7** (1 base + 6 service SDKs)
- The script **auto-bumps** all 7 crates' `version` (unless `--dry-run`,
  in which case it only prints the planned bump)
- Publish order is `digua-http-client` first, then the 6 service SDKs
- Internal `path = "../digua-http-client"` deps are temporarily rewritten
  on every service SDK crate, restored after publish

```bash
# Dry-run first
CARGO_TARGET_DIR=/tmp/sdk-target \
  pnpm run sdk:publish:rust 0.5.0 --dry-run

# Then real
CARGO_TARGET_DIR=/tmp/sdk-target \
  pnpm run sdk:publish:rust 0.5.0

# Tag
git tag v<version>-sdk
git push origin main --follow-tags
```

## Post-publish verification

```bash
# From any consumer that has CARGO_REGISTRIES_KELLNR_TOKEN set
cargo search digua_common_error --registry kellnr
cargo search digua_audit_sdk --registry kellnr

# Or via curl (kellnr exposes an HTML index)
curl -fsSL http://192.168.2.12:16444/api/v1/crates/digua_common_error | head
```

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `cargo: command not found` | Rust toolchain missing | `rustup install stable && rustup default stable` |
| `error: failed to authenticate` | Token missing/expired | `cargo login --registry kellnr` (paste token) |
| `error: failed to get ... as a dependency of package ...` | Network/DNS issue with kellnr | `curl -v http://192.168.2.12:16444/` |
| `error: API error (Forbidden): crate ... already exists at version ...` | Idempotent re-run — safe | Re-running skips it; no action needed |
| `error: failed to verify package ...` of ... `Cargo.lock` is missing | Workspace Cargo.lock not committed | `git checkout -- Cargo.lock && git commit -am "ci: refresh lockfile"` |
| Script reports `version mismatch` but proceeds anyway | Mixed versions across 17 libs | Resolve all to target version, then re-run |
| `target/` is root-owned | Docker volume race | Always use `CARGO_TARGET_DIR=/tmp/<unique-name>` |

## Rollback

crates.io and kellnr both **forbid** deletion of a published version
(within 24h on crates.io, ever on kellnr). The recovery flow:

1. Identify the broken crate(s) from the publish log
2. Bump the broken subset to a patch fix-up version (`<v>+1.0.0`)
3. Re-run the corresponding pipeline
4. Update the tag annotation (`git tag -f v<new>`) only if nothing has
   resolved it yet — warn the user before force-moving tags

## Cross-references

- **npm TS packages** (`@digua/contracts` + `@digua/http`): `pnpm run packages:publish` / CI `publish_packages`
- **Service Docker images**: see `coder-service-release` skill
- **Maven SDK** (`sdk/java/**`): see `coder-sdk-publish-maven` skill
- **Cargo workspace config**: `.cargo/config.toml` (kellnr + rsproxy mirror)
- **Why kellnr**: in-repo AGENTS.md §「网络规则」+ §「SDK 发布」
