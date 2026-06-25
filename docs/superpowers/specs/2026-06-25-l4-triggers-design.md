# P2.4 L4 Triggers Design

## Context

The project has 8 services, each with an `AGENTS.md` declaring AI conventions. P2.4 expands `.trae/rules/project_rules.md` to a comprehensive L4 trigger document and adds a pre-commit hook so violations are caught at commit-time, not just CI-time.

## Goals

1. AI agents working in this repo automatically load `.trae/rules/project_rules.md` on first file touch.
2. Path-based triggers: when the AI modifies files under a given path, the corresponding rule file or skill is auto-invoked.
3. Pre-commit hook runs the same gates as CI locally — fail-fast for developers.

## Components

### 1. `.trae/rules/project_rules.md` — Expanded L4 Trigger Document

Sections:
- **Mandatory rules** (always-on):
  - Commit policy: directly on `main`, one task per commit, no force pushes
  - Worktree policy: this repo uses main-branch workflow, no feature branches
  - Code style: AGENTS.md per service is the source of truth
  - Spec discipline: every code change must reference a spec in `docs/superpowers/specs/`
- **Path triggers**:
  - `services/*/src/routes/*.rs` → invoke `coder-add-route` skill (for new routes)
  - `services/*/migrations/**/*` → invoke `coder-migrate-db` skill (for new migrations)
  - `services/*/openapi.yaml` → run `scripts/dump-openapi-specs.sh` to verify generated spec matches
  - `contracts/**` → run `node scripts/contracts-gen.js` to regenerate consumers
  - `frontend/**` → see P2.5 specs (TODO)
- **Per-service AGENTS.md mapping** (table linking service paths to AGENTS.md files)
- **Skill discovery map** (which `.trae/skills/` to load for which task type)

### 2. `.pre-commit-config.yaml` — Pre-commit Framework Config

```yaml
repos:
  - repo: local
    hooks:
      - id: service-standard-check
        name: Service Standard Check
        entry: node scripts/check-service-standard.js
        language: node
        pass_filenames: false
        files: ^services/.*$
      - id: spec-check
        name: Spec Check
        entry: node scripts/check-spec.js
        language: node
        pass_filenames: false
        files: ^services/.*/openapi\.yaml$
      - id: contracts-check
        name: Contracts Check
        entry: node scripts/contracts-gen.js --check
        language: node
        pass_filenames: false
        files: ^contracts/.*$
```

Optional — developers can `pip install pre-commit && pre-commit install`.

### 3. `.git/hooks/pre-commit` — Direct Git Hook

Bash script that mirrors the pre-commit-config.yaml for projects that don't use the framework:

```bash
#!/bin/bash
set -e
# Run gates only if relevant paths are modified
if git diff --cached --name-only | grep -q '^services/'; then
  node scripts/check-service-standard.js || exit 1
fi
if git diff --cached --name-only | grep -q '^services/.*/openapi\.yaml$'; then
  node scripts/check-spec.js || exit 1
fi
if git diff --cached --name-only | grep -q '^contracts/'; then
  node scripts/contracts-gen.js --check || exit 1
fi
```

Hook is opt-in — developers copy it from `scripts/git-hooks/pre-commit` to `.git/hooks/pre-commit` and chmod +x.

## Why Both pre-commit-config.yaml AND a manual hook?

- `pre-commit` framework: easier to share with team via `pip install pre-commit` + `pre-commit install`
- Manual hook: zero-dependency fallback for developers who don't want pre-commit

## L4 Trigger Behavior

When an AI agent loads the repo, it reads `.trae/rules/project_rules.md` first. The path triggers tell it:

1. If user says "add a new endpoint to audit-service":
   - AI reads `services/audit-service/AGENTS.md` (service-specific rules)
   - AI loads `.agents/skills/coder-add-route/SKILL.md` (scaffold skill)
   - AI checks `services/audit-service/openapi.yaml` for existing paths

2. If user says "add a column to menu_configs":
   - AI reads `services/prue_backend/AGENTS.md`
   - AI loads `.agents/skills/coder-migrate-db/SKILL.md`
   - AI generates both MySQL and SQLite migrations

3. If user modifies `services/audit-service/openapi.yaml`:
   - AI notes that this is auto-generated and should not be edited directly
   - AI runs `scripts/dump-openapi-specs.sh` instead

## Verification

After implementing:
1. Open a new chat session in this repo
2. Verify the AI loads `.trae/rules/project_rules.md` first
3. Run `pre-commit run --all-files` (if pre-commit installed) to verify gates pass