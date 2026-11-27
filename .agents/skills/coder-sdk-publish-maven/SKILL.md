---
name: coder-sdk-publish-maven
description: |
  Publish Java SDK artifacts to the Reposilite private Maven registry.
  Covers both release (e.g. 0.4.0) and snapshot (e.g. 0.4.1-SNAPSHOT)
  flows. Use when the user says "publish java sdk", "publish maven",
  "release java sdk", "deploy maven artifacts", "sdk:publish:release",
  "sdk:publish:snapshot", "发布 java sdk", "发布 maven",
  "Reposilite release", "Reposilite snapshot", "maven deploy",
  or any equivalent intent.
---

# coder-sdk-publish-maven Skill

Publish the Java SDK multi-module artifacts (`sdk/java/`) to the
in-house **Reposilite** Maven registry. Two pipelines:

| Pipeline | Target | Version format | Trigger |
| --- | --- | --- | --- |
| **release** | Reposilite `releases` repo (`/releases`) | `X.Y.Z` (no `-SNAPSHOT`) | Local manual publish (rarely) — CI auto-publishes on `v*.*.*` tag |
| **snapshot** | Reposilite `snapshots` repo (`/snapshots`) | `X.Y.Z-SNAPSHOT` (suffix required) | Iteration between releases |

Underlying commands:

- `pnpm run sdk:publish:release` → `node packages/cli/bin/coder.js release publish maven --version X.Y.Z`
- `pnpm run sdk:publish:snapshot` → `node packages/cli/bin/coder.js release publish maven --kind snapshot --version X.Y.Z-SNAPSHOT`
- `pnpm run sdk:test:java` → `mvn -B test -f sdk/java/pom.xml` (default java8 profile)

Both thin Node scripts delegate to the retired maven-publish lib#runPublish(variant, argv)`.

## Critical invariants (read before publishing)

### A. `-SNAPSHOT` suffix is enforced by repository

Reposilite enforces the snapshot policy at the repository level:

| Target repo | Accepts `-SNAPSHOT`? | Rejects `-SNAPSHOT`? |
| --- | --- | --- |
| `releases` | no (rejects all `-SNAPSHOT`) | yes |
| `snapshots` | yes | no (rejects non-`-SNAPSHOT`) |

The script **hard-exits 3** if you mix them up. Pre-flight catches this
before any network traffic.

### B. pom.xml bump is NOT auto-committed

After running `publish-maven-release.js`, `sdk/java/pom.xml`'s
`<version>` is rewritten via `mvn versions:set` (idempotent). The script
prints the reminder but does **not** `git commit`. You must:

```bash
git diff sdk/java/                       # review the version bump
git add sdk/java/
git commit -m "feat(sdk): bump to <version>"
```

If you skip this, the next publish run will see pom != requested
version, attempt to re-bump, and you'll have uncommitted drift on disk.
More dangerously: any consumer that builds from source (`mvn install`
locally before consuming) will get a version mismatch with the jar
on the registry.

### C. Default upload mode is `curl`, not `mvn deploy`

Reposilite + Maven Aether HttpClient has a known **Expect:100-continue**
incompatibility that causes HTTP 400 on `mvn deploy`. The publish script
works around this with a 2-step `curl -T` upload:

1. `mvn install` → populate `~/.m2/repository/io/digua/**/<version>/`
2. `curl -T` each `.pom` / `.jar` (with `.md5` / `.sha1` / `.sha256`
   sidecar files) to Reposilite

Use `--mvn` only if Reposilite is fixed upstream; default is correct for
our environment.

### D. Release mode is idempotent (HEAD-SKIP), snapshot is not

- **release**: script HEAD-checks each artifact; if it exists on
  Reposilite, **SKIP** without overwriting. Re-running the same release
  version is safe (no-op after first successful run).
- **snapshot**: no HEAD check; Reposilite snapshot policy allows
  overwrite (this is the snapshot contract).

### E. Credentials cascade (4 levels)

| Priority | Source | How to set |
| --- | --- | --- |
| 1 | CLI args `--user --pass` | `node ... --user admin --pass '***'` |
| 2 | env `MAVEN_USERNAME` + `MAVEN_PASSWORD` | `export MAVEN_USERNAME=...` |
| 3 | Wizard config `infrastructure/.docker-registry.json` | `pnpm run setup:wizard` (authMode = userpass / token) |
| 4 | `~/.m2/settings.xml` `<server id="reposilite-repository-releases">` | Standard Maven settings |

For CI: use env vars (level 2) so secrets stay in CI variables, not in
git-tracked files.

## Pre-flight (mandatory)

```bash
bash .agents/skills/coder-sdk-publish-maven/scripts/preflight.sh \
  <release|snapshot> <version>
```

The pre-flight checks:

1. `mvn --version` ≥ 3.9, on PATH
2. `curl --version` on PATH (default mode only — skip if you plan `--mvn`)
3. Java JDK ≥ 8 on `JAVA_HOME` (or `java` on PATH)
4. `sdk/java/pom.xml` is well-formed XML
5. All 7 expected modules exist with `pom.xml`:
   `common`, `digua-user-user-sdk`, `digua-user-client-sdk`,
   `digua-token-user-sdk`, `digua-token-client-sdk`, `digua-http-client`,
   `digua-notification-sdk`
6. `<groupId>` in pom is `io.github.luqizheng`
7. Current pom `<version>` is in the expected track (release / snapshot)
8. Reposilite base URL is reachable (`curl --max-time 5 -o /dev/null -w '%{http_code}'`)
9. At least one credential source is available (CLI / env / wizard /
   `~/.m2/settings.xml`); otherwise exit 1 with instructions
10. `git status` reports whether `sdk/java/` has uncommitted changes

The script aborts on any ERROR. WARN items (e.g. uncommitted drift,
missing optional credential sources) are surfaced but not blocking.

## Release Flow — snapshot (most common in dev)

### Step 1 — Confirm intent + version

Ask the user:

1. **Next snapshot version** — default increment is
   `<current>-SNAPSHOT` → `<current + 1>-SNAPSHOT`. E.g. `0.4.1-SNAPSHOT`.
2. **Java profile** — `java8` (default) / `java17` / `java21` (bytecode
   target; builder image is always JDK 21).
3. **Modules subset** (optional) — `--modules "common,user-service-user-sdk"`
   to publish only a subset (saves time during iteration).

Display planned command and wait for explicit confirmation.

### Step 2 — Pre-flight

```bash
bash .agents/skills/coder-sdk-publish-maven/scripts/preflight.sh \
  snapshot 0.4.1-SNAPSHOT
```

### Step 3 — Tests (mandatory)

```bash
# All modules
pnpm run sdk:test:java

# Or profile-specific
pnpm run sdk:test:java:17
```

If tests fail, **stop**. Do not push a broken artifact.

### Step 4 — Dry-run (strongly recommended first time)

```bash
pnpm run sdk:publish:snapshot \
  --version 0.4.1-SNAPSHOT \
  --dry-run \
  --profile java17
```

`--dry-run` runs `mvn clean verify` (no deploy) and the
`mvn versions:set` in dry-run echo mode — no artifacts reach Reposilite,
no `~/.m2/repository/io/digua/**/<version>/` files are produced.

### Step 5 — Real publish

```bash
pnpm run sdk:publish:snapshot --version 0.4.1-SNAPSHOT --profile java17
```

The script will:

1. Set `sdk/java/pom.xml` `<version>` to `0.4.1-SNAPSHOT`
2. Run `mvn -Pjava17 clean install`
3. Walk `~/.m2/repository/io/digua/**/0.4.1-SNAPSHOT/`, upload every
   `.pom` + `.jar` (+ `.md5` / `.sha1` / `.sha256`) via `curl -T`
4. Print "snapshot deploy complete (curl mode)"

### Step 6 — Commit the pom.xml change

```bash
git diff sdk/java/
git add sdk/java/
git commit -m "feat(sdk): bump to 0.4.1-SNAPSHOT"
```

**Do NOT skip this.** Without the commit, the next publish run sees
pom != requested version and re-bumps.

### Step 7 — Verify on Reposilite

```bash
# List metadata
curl -fsSL http://192.168.2.12:18181/snapshots/io/digua/digua-sdk-parent/maven-metadata.xml
# Should show <version>0.4.1-SNAPSHOT</version>

# Pull specific module POM
curl -fsSL -o /tmp/digua-sdk-common.pom \
  http://192.168.2.12:18181/snapshots/io/digua/digua-sdk-common/0.4.1-SNAPSHOT/digua-sdk-common-0.4.1-SNAPSHOT.pom
xmllint --noout /tmp/digua-sdk-common.pom  # validate XML
```

## Release Flow — release (CI-driven, manual is rare)

CI auto-publishes on `v*.*.*` git tag (see `.gitlab-ci.yml`).

If you must publish locally (debug, dry-run of a release):

```bash
# Step 1 — Pre-flight
bash .agents/skills/coder-sdk-publish-maven/scripts/preflight.sh release 0.4.0

# Step 2 — Dry-run
pnpm run sdk:publish:release --version 0.4.0 --dry-run

# Step 3 — Real
pnpm run sdk:publish:release --version 0.4.0

# Step 4 — Commit pom bump
git add sdk/java/ && git commit -m "feat(sdk): bump to 0.4.0"

# Step 5 — Tag
git tag v0.4.0
git push origin main --follow-tags   # CI picks up v0.4.0 tag and re-publishes
```

## Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `ERROR: 'mvn' not found on PATH` | Maven not installed | `brew install maven` / `apt install maven` / download tarball |
| `ERROR: --version 'X.Y.Z-SNAPSHOT' ends with -SNAPSHOT` (release) | Used snapshot version with release command | Re-run with `--version 0.4.0` |
| `ERROR: --version 'X.Y.Z' does NOT end with -SNAPSHOT` (snapshot) | Used release version with snapshot command | Re-run with `--version 0.4.1-SNAPSHOT` |
| HTTP 400 from Reposilite on `mvn deploy` | Known Aether Expect:100-continue bug | Default is curl mode; if you forced `--mvn`, drop the flag |
| `ERROR: no artifacts found under ~/.m2/repository/io/digua/...` | `mvn install` didn't produce files (profile / modules mismatch) | Check `--profile` matches modules' expected profile; verify `mvn install` succeeded |
| `SKIP <file>` printed for every file (release mode) | Re-running same release — expected | No action needed (idempotent) |
| pom.xml drift on disk (not committed) after publish | Forgot Step 6 | `git diff sdk/java/` then `git commit` |
| `error: 401 Unauthorized` curl upload | Wrong credential source selected | Check `~/.m2/settings.xml` `<server id="reposilite-repository-releases">` matches `MAVEN_REPO_ID`; or pass `--user --pass` explicitly |
| Module `notification-service-sdk` not in repo | README lists it; pom.xml `<modules>` doesn't | README is stale (0.2.0 docs vs 1.1.0 pom). The skill warns if a README-listed module has no pom |

## Repository URL drift (informational)

There are **two URL families** in this repo, and they don't currently
agree:

| Source | Release URL | Snapshot URL |
| --- | --- | --- |
| `sdk/java/pom.xml` `<distributionManagement>` | `http://192.168.2.12:18181/releases` | `http://192.168.2.12:18181/snapshots` |
| `sdk/java/README.md` "仓库" section | `http://mvn.kudi.repos/repository/maven-releases/` | `http://mvn.kudi.repos/repository/maven-snapshots/` |
| the retired maven-publish lib `DEFAULT_REPO_URL` | `http://192.168.2.12:18181/releases` | `http://192.168.2.12:18181/snapshots` |

**Authoritative source is the pom.xml `<distributionManagement>` and
the script's `DEFAULT_REPO_URL`** — both agree on
`192.168.2.12:18181`. The README's `mvn.kudi.repos` URLs appear to be
an aspirational DNS alias that hasn't been wired up yet. Use `--repo-url`
to override if you need to test against `mvn.kudi.repos`.

## Rollback

Reposilite has a configurable retention policy (default: keep all
artifacts forever). The recovery flow:

1. **snapshot**: just publish a newer snapshot (`X.Y.Z+1-SNAPSHOT`) —
   consumers will pull the newer one (Reposilite serves newest by
   maven-metadata).
2. **release**: **immutable** by Maven convention. Republish a fixed
   version (e.g. `0.4.1` instead of `0.4.0`) — do NOT delete `0.4.0`.

There is no `mvn undeploy` workflow here. Don't try to remove broken
release artifacts.

## Maven Central — public publication

The public Maven Central deployment is separate from the internal Reposilite workflow above.

- Maven coordinates now use `io.github.luqizheng`; Java source packages remain `io.digua.*`.
- Dedicated workflow: `.github/workflows/java-sdk-maven-central.yml`.
- Only strict `vMAJOR.MINOR.PATCH` tags trigger Central upload. PRs and branch pushes only verify.
- The workflow requires `MVN_USERNAME`, `MVN_PASSWORD`, `GPG_PRIVATE_KEY`, and `GPG_PASSPHRASE`. GPG secrets are not ready; do not create a release tag until configured.
- Central Publishing uses `autoPublish=false`. After upload and validation, a maintainer must manually click Publish in Sonatype Central.
- Never run `mvn deploy` with the Central profile during validation. Never create a release tag without explicit user authorization.
- The Central publishing guide is `docs-site/guides/java-sdk-publishing.md`.

For an explicit internal Maven deploy (not the normal curl-based CLI flow), activate
`-Preposilite-publishing`. This profile is intentionally inactive during Central
publication so the private Reposilite URL is not embedded in public POM metadata.

## Cross-references

- **npm TS packages** (`@digua/contracts` + `@digua/http`): `pnpm run packages:publish` / CI `publish_packages`
- **Rust crates** (libs + sdks): see `coder-sdk-publish-rust` skill
- **Service Docker images**: see `coder-service-release` skill
- **CI auto-publish**: `.gitlab-ci.yml` (release tag → publish job)
- **Full publishing spec**: `docs-site/guides/java-sdk-publishing.md`
- **Module list drift**: `sdk/java/pom.xml` is authoritative
- **Why curl not mvn deploy**: this SKILL.md §C
