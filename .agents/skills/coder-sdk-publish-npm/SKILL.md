---
name: coder-sdk-publish-npm
description: |
  Publish the @digua/dist-tools npm package to the private npm.kudi.repos
  registry. Use when the user says "publish npm sdk", "release dist-tools",
  "sdk:publish:npm", "publish @digua/dist-tools", "发布 npm 包", "打 npm 包",
  "上传 dist-tools", or any equivalent intent. Cross-platform (Linux / macOS
  / Windows); the underlying npm CLI invocation is the same on every OS.
---

# coder-sdk-publish-npm Skill

Publish the in-repo npm SDK package — `@digua/dist-tools` — to the private
`npm.kudi.repos` registry. The package is consumed by every downstream
repository (e.g. `project-manager`) via `npx cms-sync` to apply
upstream-synced release tarballs.

## Scope

| What this skill does | What it does NOT do |
| --- | --- |
| Bump + publish **only** `release/dist-tools/` | Maven SDK (`sdk/java/**`) — use `pnpm run sdk:publish:release / snapshot` |
| Increment `release/dist-tools/package.json` `version` | Rust SDK / libs — use `coder-sdk-publish-rust` skill |
| Run `npm test` then `npm publish` | Service Docker images — use `coder-service-release` skill |
| Verify the published version on the registry | Bump the cross-repo sync lock file (downstream's `.upstream.lock`, not in this repo) |

## Pre-flight (run before bumping version)

Ask the user for:

1. **Next version** — semver bump from current. Confirm with the user
   whether it's `patch` / `minor` / `major`. Default hint: most recent git
   tag on `release/dist-tools/**` (if any) plus one semver step.
2. **Release channel** — `npm.kudi.repos` only (no public registry, the
   package is `UNLICENSED` / internal-use-only).

Then run `scripts/preflight.sh <next-version>` (provided with this skill) to
verify all of the following. **Do not proceed if any check fails.**

```bash
bash .agents/skills/coder-sdk-publish-npm/scripts/preflight.sh <next-version>
```

Pre-flight covers:

- `release/dist-tools/package.json` exists and parses as JSON
- `name` field is exactly `@digua/dist-tools`
- `publishConfig.registry` is `http://npm.kudi.repos/`
- `version` in `package.json` is lower than `<next-version>`
- Working tree is clean (no uncommitted changes under `release/dist-tools/`)
- `npm` CLI is on PATH and is v9+
- Private registry `npm.kudi.repos` is reachable (HTTP HEAD)
- Auth is configured (presence of `~/.npmrc` token for `npm.kudi.repos` or
  env var `NPM_CONFIG_REGISTRY`); the script prints instructions if not

## Release Flow

### Step 1 — Confirm intent

Surface the diff and the publish preview to the user **before** running
`npm publish`. Show:

- Current version → next version
- Files that will be packed (`npm pack --dry-run` output)
- The exact publish command that will run

Wait for explicit confirmation.

### Step 2 — Bump version

```bash
# from repo root
npm version <next-version> --prefix release/dist-tools \
  --no-git-tag-version --allow-same-version
```

`--no-git-tag-version` because the tag should be at repo root after the
publish, not inside `release/dist-tools/`. The next step creates the tag.

### Step 3 — Run unit tests (mandatory)

```bash
npm test --prefix release/dist-tools
```

If tests fail, **stop**. Do not publish a broken package. Investigate,
fix, re-run.

### Step 4 — Dry-run (strongly recommended)

```bash
npm publish --prefix release/dist-tools --dry-run
```

Inspect the packed tarball listing; ensure no unintended files leak into
the published artifact (the existing `files` field in `package.json`
already restricts to `bin/`, `lib/`, `README.md`).

### Step 5 — Publish

```bash
pnpm run sdk:publish:npm
```

This is equivalent to `npm publish --prefix release/dist-tools` (see
`package.json` script definition).

If the registry returns `ENEEDAUTH` (HTTP 401), check `~/.npmrc`:

```ini
registry=http://npm.kudi.repos/
//npm.kudi.repos/:_authToken=${NPM_TOKEN}
```

The script does NOT auto-recover from auth failure — surface the error to
the user and let them refresh the token.

### Step 6 — Tag the release in git

```bash
git add release/dist-tools/package.json
git commit -m "release(dist-tools): <next-version>"
git tag v<next-version>
git push origin main --follow-tags
```

Tag convention: `v<version>` (with `v` prefix). The downstream
`.upstream.lock` references this tag.

### Step 7 — Verify

```bash
npm view @digua/dist-tools versions --registry=http://npm.kudi.repos/ \
  | tail -5
```

Confirm the new version appears in the tail of the list.

## Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `npm ERR! code E404` on dry-run | Wrong registry or token | Verify `~/.npmrc` `registry=` line points to `npm.kudi.repos` |
| `npm ERR! code E403` on publish | Version already exists / no publish permission | Confirm `version` is strictly greater than the latest on registry; ask registry owner to grant publish |
| `npm ERR! code ENEEDAUTH` | Missing / expired token | Re-login: `npm login --registry=http://npm.kudi.repos/` (interactive) or inject `NPM_TOKEN` |
| `npm WARN tarball ... no holo` | Older npm (<9) | `npm install -g npm@latest` |
| Tests pass locally but tarball contains stale files | Working tree had uncommitted edits at publish time | Re-run pre-flight; commit clean working tree first |

## Rollback

`npm unpublish` is only allowed within **72 hours** of publish (npm
policy). Beyond that, the only recovery is to publish a fix-up patch
version (`<version>+1.patch.0`) and re-tag.

If a tag was pushed but the registry is wrong, **do not** force-push the
tag without first warning the user — downstream consumers may have
already resolved it.

## Cross-references

- **Rust crates** (libs + sdk): see `coder-sdk-publish-rust` skill
- **Service Docker images** (compose + airgap tar): see `coder-service-release` skill
- **Maven SDK** (`sdk/java/**`): out of scope for this skill — run `pnpm run sdk:publish:release` or `pnpm run sdk:publish:snapshot`
- **Why this package exists**: `release/dist-tools/README.md` §「解决什么问题」
- **Downstream consumption**: `cms-sync` in `bin/sync.js`
