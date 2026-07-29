# Project Rules — L4 Trigger Document

This file is the top-level AI trigger document for the `coder-micro-services` repository.
AI agents MUST load this file on first file touch in this repo.

## 0. AI Entry Point

**Read `docs-site/ai-context.md` FIRST** before loading per-service AGENTS.md or any spec.
`ai-context.md` is the single source of truth for project shape, conventions, known pitfalls, and skill catalog.

---

## 1. Mandatory Rules (Always On)

### 1.1 语言与文档

- AI 的**思考过程、文档撰写、对话回复必须使用中文**。代码注释、commit message、技术术语保持英文。
- 新写或重写的文档、spec、设计说明默认中文；既有英文文档（如 `docs-site/ai-context.md`）下次大改时一并转中文。
- 本规则对 `.trae/rules/*` 下所有规则文件统一生效。

### 1.2 Commit & Branch Policy

- **Use task branches for feature/validation work.** Create a descriptive branch from `main` (for example `validation/software-project-management` or `feat/<scope>`), keep it synced with `main` as needed, and merge back only after gates pass.
- **One task per commit.** Don't bundle unrelated changes.
- **Never amend or force-push.** If a commit fails, create a new one.
- **Commit message format**: `<Phase>:<short imperative summary>` or `feat(scope): <summary>`.
  - Phase tags: `P1.1`, `P1.2`, ..., `P2.3`, `P3.1`
  - Conventional tags: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`
- **No secrets in commits.** `.env`, `*.pem`, `keys/`, `**/secret*` are git-ignored.

### 1.3 Spec Discipline

- Every code change MUST reference a spec file in `docs/superpowers/specs/`.
- Spec must be created BEFORE code (in the same commit or a prior commit).
- Spec docs follow the convention `YYYY-MM-DD-<topic>-design.md`.

### 1.4 Code Style

- Each Rust service has its own `AGENTS.md` declaring its conventions. **Read it before editing.**
- Each Rust service has a Dockerfile. Docker base image lives at `infrastructure/docker/base-images/<svc>/Dockerfile`.
- Migrations go in `services/{svc}/migrations/{mysql,sqlite}/` (or `services/{svc}/migrations/` for legacy flat layout).

### 1.5 CI Gates (must pass before push)

| Gate | Script | Triggers |
|------|--------|----------|
| Service Standard | `node scripts/check-service-standard.js` | `services/**` |
| Spec Check | `node scripts/check-spec.js` | `services/**/openapi.yaml` |
| Contracts Check | `node scripts/contracts-gen.js --check` | `contracts/**` |
| OpenAPI Drift | `scripts/dump-openapi-specs.sh` + `git diff` | `services/*/openapi.yaml` |
| Rust Tests | `node scripts/run-tests-docker.js <svc>` | `services/<svc>/**` |

All gates must pass locally before pushing. Run them with:
```bash
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
node scripts/run-tests-docker.js audit-service config-service file-service prue_backend iam-service notification-service token-service
```

---

## 2. Path-Based Triggers

When the AI works in any of the following paths, it MUST also load the referenced file or skill:

### 2.1 Service Routes — `services/*/src/routes/*.rs`

- Read `services/<svc>/AGENTS.md` for service-specific endpoint conventions.
- Load skill `.agents/skills/coder-add-route/SKILL.md` if creating a NEW route.
- After adding a route, update `services/<svc>/src/openapi.rs` with the handler and any new DTOs.
- Run `scripts/dump-openapi-specs.sh` to regenerate `openapi.yaml`.

### 2.2 Database Migrations — `services/*/migrations/**/*.sql`

- Read `services/<svc>/AGENTS.md` for service-specific DB conventions.
- Load skill `.agents/skills/coder-migrate-db/SKILL.md` if creating a NEW migration.
- Generate BOTH MySQL AND SQLite versions unless the service uses a flat `migrations/` directory.
- Never edit existing migrations. Create a new one instead.

### 2.3 OpenAPI Specs — `services/*/openapi.yaml`

- These files are **auto-generated**. Do NOT edit them directly.
- To regenerate: `scripts/dump-openapi-specs.sh`.
- The CI gate `spec_generate` will fail if `openapi.yaml` differs from what utoipa produces.

### 2.4 Contracts — `contracts/**`

- This is the single source of truth for error codes, permissions, events, HTTP conventions.
- Edit YAML in `contracts/` only.
- After editing, run `node scripts/contracts-gen.js` to regenerate consumers in `libs/common-error/src/codes.rs` and `libs/authz/src/perms.rs`.
- The `contracts_check` gate fails if generated code drifts from YAML.

### 2.5 Frontend — `frontend/**`

- See `.trae/rules/frontend-rules.md` (TODO P2.5).
- Vue 3 + Vite + Pinia. Frontend contracts live in `frontend/src/contracts/`.

### 2.6 Specs & Plans — `docs/superpowers/specs/`, `docs/superpowers/plans/`

- Read the related spec BEFORE implementing changes.
- Spec docs MUST be created in the same branch / commit cycle as the implementation.
- Plan docs track multi-task rollouts.

---

## 3. Per-Service AGENTS.md Mapping

AI MUST read the AGENTS.md file when modifying files under any of these paths:

| Service Path | AGENTS.md Location |
|--------------|--------------------|
| `services/audit-service/**` | `services/audit-service/AGENTS.md` |
| `services/config-service/**` | `services/config-service/AGENTS.md` |
| `services/file-service/**` | `services/file-service/AGENTS.md` |
| `services/iam-service/**` | `services/iam-service/AGENTS.md` |
| `services/notification-service/**` | `services/notification-service/AGENTS.md` |
| `services/prue_backend/**` | `services/prue_backend/AGENTS.md` |
| `services/token-service/**` | `services/token-service/AGENTS.md` |
| `services/api-gateway/**` | `services/api-gateway/AGENTS.md` |
| `frontend/**` | `frontend/AGENTS.md` (TODO P2.5) |
| `libs/**` | `libs/AGENTS.md` (if exists; otherwise read README) |
| `contracts/**` | `contracts/AGENTS.md` (TODO) |

---

## 4. Skill Discovery Map

| User Intent | Skill to Load |
|-------------|---------------|
| Add a route/endpoint | `.agents/skills/coder-add-route/SKILL.md` |
| Add a DB migration | `.agents/skills/coder-migrate-db/SKILL.md` |
| Create a new Rust service | `.agents/skills/coder-new-service/SKILL.md` |
| Review a plan | `.agents/skills/plan-eng-review/SKILL.md` |
| Sync base Docker images | `.trae/skills/coder-base-images-sync/SKILL.md` |
| Bootstrap dev env | `.trae/skills/coder-dev-bootstrap/SKILL.md` |
| Audit service standards | `.trae/skills/coder-service-audit/SKILL.md` |
| Run e2e tests | `.trae/skills/coder-e2e-test/SKILL.md` |
| Pull docker images | `.trae/skills/coder-docker-pull/SKILL.md` |
| Upgrade library | `.trae/skills/coder-lib-upgrade/SKILL.md` |

---

## 5. Local Development Setup

1. **Pre-commit hooks (recommended)**:
   ```bash
   pip install pre-commit
   pre-commit install
   ```
   Config lives at `.pre-commit-config.yaml` in repo root.

2. **Direct git hook (fallback)**: copy `scripts/git-hooks/pre-commit` to `.git/hooks/pre-commit` and `chmod +x` it.

3. **Run gates locally**:
   ```bash
   node scripts/check-service-standard.js
   node scripts/check-spec.js
   node scripts/contracts-gen.js --check
   scripts/dump-openapi-specs.sh
   git diff --exit-code -- 'services/*/openapi.yaml' || echo "drift detected"
   ```
4. **WSL Development (Windows users, optional)**:
   - Windows 开发者**推荐**使用 WSL2 环境（非强制，原生 PowerShell / CMD 也可）
   - 详细规则见 .trae/rules/wsl-development-rules.md

---

## 6. Reference Documents

- `docs/superpowers/plans/2026-06-25-overall-roadmap.md` — 12-week master plan
- `docs-site/decisions/ADR-002-service-standard-baseline.md` — service standard baseline
- `docs/superpowers/specs/` — design specs (one per change)
- `docs/superpowers/plans/` — multi-task rollouts
- `contracts/` — error codes / permissions / events source of truth
- `TODO.md` — current debt and pending items