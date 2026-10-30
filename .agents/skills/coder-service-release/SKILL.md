---
name: coder-service-release
description: |
  Release a versioned Docker image bundle for all in-repo services and
  produce either a full airgap tarball or a slim deploy-only bundle for
  downstream consumers. Wraps the build → push → transform-compose →
  airgap-tar pipeline with cross-repo safety checks (compose command
  naming, [[bin]] kebab → underscore, infrastructure/ AGENTS.md L1/L2).
  Use when the user says "release service images", "release:build",
  "release:deploy", "ship a version", "打镜像包", "打 airgap 包",
  "服务发布", "release v1.x.x", "打包离线分发包", "打包纯部署包",
  or any equivalent intent.
---

# coder-service-release Skill

End-to-end release of the **service image bundle** consumed by downstream
deployments (e.g. `project-manager`). Since 2026-10-30 downstream updates are
package-only (image tag bump + cargo/maven/pnpm version bump); the
source-level sync tool `cms-sync` is retired. The bundle contains:

- Docker images for all in-repo services + `mysql-replica`
  (pushed to `zhcoder-docker-registry.com:8000/micro-server/*`)
- A rendered `infrastructure/docker-compose-{services,main}.yml` with
  `build:` blocks **replaced by `image:` references** (so downstream
  doesn't try to rebuild from source)
- The dev skeleton (contracts / scripts / skills / frontend source /
  services / Cargo.toml / package.json) so downstream can run e2e

## Two flavors of bundle

| Flavor | Command | What's inside | When to use |
| --- | --- | --- | --- |
| **airgap** (full) | `pnpm run release:build` | image tarballs + dev skeleton + docs | First install of a downstream consumer, or major upgrade |
| **deploy-only** (slim) | `pnpm run release:deploy` | image tarballs + slim ops scripts only | Patch-level updates to existing downstream (no dev tooling needed) |

Both call `release/dist-template/scripts/{release.js,release-deploy.js}`.
Both run `release/dist-template/scripts/transform-compose.js` to convert
`build:` → `image:`. Both honor `REGISTRY` env var (default
`zhcoder-docker-registry.com:8000`).

## Critical constraints (read before any release)

### A. Cargo `[[bin]]` → container binary naming

Cargo normalizes kebab-case `[[bin]] name` to **underscore** in the
container's `/usr/local/bin/`. This is documented in
`AGENTS.md` §「跨仓库分发」(cross-repo distribution).

```toml
# services/notification-service/Cargo.toml
[[bin]] name = "notification-service"  # 实际产物 /usr/local/bin/notification_service
[[bin]] name = "ntf-outbox-publisher"  # 实际产物 /usr/local/bin/ntf_outbox_publisher
```

Therefore docker-compose `command:` fields **must use underscores**, not
kebabs:

```yaml
# ✅ correct
notification-service:
  command: ["notification_service"]   # underscore

ntf-outbox-publisher:
  command: ["ntf_outbox_publisher"]   # underscore

# ❌ wrong — downstream reports OCI 'executable file not found in $PATH'
notification-service:
  command: ["notification-service"]   # kebab
```

The pre-flight script `scripts/check-compose-commands.sh` (shipped
with this skill) walks every `services.<svc>` in
`infrastructure/docker-compose-services.yml`, resolves each `command:`
against the corresponding `Cargo.toml` `[[bin]] name`, and fails the
release if any mismatch is detected. **Run it before every release.**

### B. Cross-repo distribution risk

New environments bootstrap from the air-gap bundle, and its compose is
generated from this repo. A broken compose `command:` shipped in the
bundle will affect every new deployment until a fixed bundle is cut.
Existing deployments own their compose files — they only bump image tags
— so a compose bug does not retroactively change running environments.

Therefore:

- `infrastructure/docker-compose-services.yml` and
  `infrastructure/docker-compose.yml` are **release-blocker files**.
- Any change to either goes through `infrastructure/AGENTS.md` L1 → L2
  loop: report-only in week one, auto-fix thereafter. During L1, this
  skill must **not** auto-edit compose files; it must surface the diff
  and require human approval before `release.js` writes.

### C. `--no-images` and `--images` flags

Both `release.js` and `release-deploy.js` accept:

- `--no-images` / `EXPORT_IMAGES=0`: skip `docker pull` + `docker save`,
  bundle skeleton only. Useful when you're CI without Docker daemon or
  when you trust the registry to deliver images via `load-images.js
  --mode=pull`.
- `--images` / `EXPORT_IMAGES=1`: default. Bundles images as tar.

Use `--no-images` for **dry-run** or skeleton-only changes (e.g.
fixing a compose typo without re-pulling 10 GB of images).

## Pre-flight (mandatory)

```bash
bash .agents/skills/coder-service-release/scripts/preflight.sh
```

The pre-flight checks:

1. `docker` CLI on PATH, daemon reachable
2. `node` >= 18 on PATH
3. `git` working tree is clean (a release cut should be from a tagged
   commit, not a WIP)
4. No uncommitted changes under `infrastructure/` (forces the L1 loop
   to have completed)
5. SCAN: `check-compose-commands.sh` against
   `infrastructure/docker-compose-services.yml`
6. SCAN: every `services.*` referenced in compose has a matching
   `services/<svc>/Cargo.toml` (catches typos like `iam-srevice`)
7. SCAN: `infrastructure/.env` exists and has all required vars (the
   compose templates reference them via `${...}`)
8. REPORT: print the candidate `REGISTRY` and `VERSION` to be used

The script aborts on any ERROR, but treats L1-loop "warnings" as soft
(git status, env completeness) so the human can decide.

## Release Flow

### Step 1 — Confirm intent + version

Ask the user for:

1. **Version tag** — semver, e.g. `v1.2.0` or `v1.2.0-rc.1`
2. **Bundle flavor** — `airgap` (full, `release:build`) or `deploy` (slim,
   `release:deploy`)
3. **Image strategy** — bundle images (`--images`) or skeleton-only
   (`--no-images`)
4. **Push to registry?** — if yes, run `docker:build --push` first;
   the bundle will then `docker pull` from the same registry.

Display the planned command line before running:

```
node release/dist-template/scripts/release.js v1.2.0 \
  --images \
  REGISTRY=zhcoder-docker-registry.com:8000
```

### Step 2 — Pre-flight

```bash
bash .agents/skills/coder-service-release/scripts/preflight.sh
```

### Step 3 — (Optional) dry-run

```bash
EXPORT_IMAGES=0 node release/dist-template/scripts/release.js v1.2.0 --no-images
```

This builds the skeleton, transforms compose, writes MANIFEST.json, but
**skips** `docker pull` + `docker save`. Useful when iterating on
infrastructure-only changes; avoids 10+ GB of network/disk.

### Step 4 — (If pushing images) Build & push first

```bash
# Multi-arch example (Linux/macOS only)
pnpm run docker:build -- --platform linux/amd64,linux/arm64 --push -v 1.2.0

# Single-arch, keep local + push
pnpm run docker:build -- --load-and-push -v 1.2.0
```

Wait for all images to land on
`zhcoder-docker-registry.com:8000/micro-server/<svc>:1.2.0`.

### Step 5 — Real bundle

```bash
# Full airgap
pnpm run release:build

# Slim deploy-only
pnpm run release:deploy
```

Both print the output tarball path:

```
release/coder-micro-services-v1.2.0-airgap.tar.gz
release/coder-micro-services-v1.2.0-deploy.tar.gz
```

### Step 6 — Verify the bundle locally

```bash
# Extract and inspect MANIFEST.json
mkdir -p /tmp/release-check && cd /tmp/release-check
tar -xzf <repo>/release/coder-micro-services-v1.2.0-airgap.tar.gz
cat coder-micro-services-v1.2.0/MANIFEST.json | jq .

# Confirm compose was transformed
grep -E '^\s+image:' coder-micro-services-v1.2.0/infrastructure/docker-compose-services.yml
```

If any line still shows `build:` instead of `image:`, the bundle is
broken — re-run `release.js` (it always calls `transform-compose.js`).

### Step 7 — Tag the release

```bash
git tag v1.2.0
git push origin main --follow-tags
```

### Step 8 — Notify downstream consumers

Downstream consumers do **not** sync sources. They consume the release as
packages: bump `IMAGE_TAG` in compose `.env` and `docker compose pull && up -d`,
then bump crate / Maven / npm versions as needed (same tag = same version
across all registries). See `docs-site/guides/upstream-sync.md`.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `executable file not found in $PATH` in downstream container | compose `command:` used kebab-case | Fix compose → re-bundle → re-publish |
| Bundle is missing `images/*.tar` | Ran with `--no-images` | Re-run without the flag, or document that consumer must `docker pull` |
| `transform-compose` reports "未找到需要替换的 build: 块" | compose was already transformed on a previous run | Don't pre-transform; let `release.js` do it once |
| `docker pull` hangs | Registry network issue | Verify `zhcoder-docker-registry.com:8000` reachable; check VPN if applicable |
| `registry/micro-server/<svc>:<ver>` returns 404 | Image wasn't pushed in Step 4 | Re-run `pnpm run docker:build -- --push -v <ver>` |
| `release.js` failed at `transform-compose` for `mainCompose` | Main `docker-compose.yml` missing | Run from repo root; do not invoke `release.js` from elsewhere |

## L1 / L2 loop compliance

Per `infrastructure/AGENTS.md`:

- **L1 (week one, report-only):** this skill surfaces diffs, does NOT
  edit compose files. Human applies the fix, runs the skill again.
- **L2 (auto-fix enabled):** this skill may auto-edit compose files,
  but must still emit a change summary for human review.

The pre-flight script respects the current L-level by switching between
`--strict` (L1) and `--auto-fix` (L2) modes. Default is `--strict`.

## Cross-references

- **npm TS packages** (`@digua/contracts` + `@digua/http`): `pnpm run packages:publish` / CI `publish_packages`
- **Rust crates** (libs + sdks): see `coder-sdk-publish-rust` skill
- **Maven SDK** (`sdk/java/**`): see `coder-sdk-publish-maven` skill
- **Why this matters** (cross-repo risk): `AGENTS.md` §「跨仓库分发」
- **Loop conventions**: `infrastructure/AGENTS.md` (Test: `npm test` / `npm run lint`;
  Loop: report-only L1 → auto-fix L2)
- **Downstream consumption**: package-only updates, see `docs-site/guides/upstream-sync.md`
- **Release bundle scripts**: `release/dist-template/scripts/`
