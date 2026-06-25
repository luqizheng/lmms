# P3.5 Knowledge Consolidation Design

## Context

The codebase has matured through Phase 1–3: 8 services, utoipa-driven OpenAPI, smoke-tested e2e, pre-commit hooks, L4 triggers. New AI instances (or new human devs) opening the repo need an **AI-focused entry point** to get productive in <5 minutes — without having to read every ADR/spec/skill.

This addresses the long-term AI efficiency plan's Phase 4 goal: *"project's own knowledge feeds back into AI."*

## Decision

### 1. New file: `docs/ai-context.md`

Single-file AI entrypoint with these sections:

1. **Project Snapshot** — one-paragraph description, repo layout, current phase
2. **Service Map** — table of all 8 services with port, db, deps
3. **Architecture** — ASCII diagram of request flow + layer responsibilities
4. **ADR Index** — short summary of each ADR with one-line lesson
5. **Spec Index** — list of all current specs by topic
6. **Skill Catalog** — one-line per skill in `.agents/` and `.trae/`
7. **Known Pitfalls** — historical bugs / gotchas (from real experience)
8. **Common Commands** — the 10 commands devs run most
9. **CI Gates** — what's checked and how to run locally

### 2. Reference chain update

`.trae/rules/project_rules.md` adds:

> "AI agents MUST read `docs/ai-context.md` first, before `AGENTS.md` per service."

This makes `ai-context.md` the **first** doc the AI loads, replacing the previous "load project_rules.md" behavior.

### 3. Monthly retrospective template

`docs/superpowers/retros/_template.md` provides a YAML-based retrospective format with sections:
- Specs completed this month
- Skills added / updated
- ADR updates
- Common failure modes
- Next month goals

First retrospective: `docs/superpowers/retros/2026-06-retrospective.md` capturing the June 2026 work.

### 4. Service map data

Sourced from per-service `AGENTS.md` files (already standardized in P1.3). The ai-context.md hand-codes a summary table cross-referencing those files.

## Files Touched

| File | Change |
|------|--------|
| `docs/ai-context.md` | NEW — AI entrypoint |
| `docs/superpowers/specs/2026-06-25-knowledge-consolidation-design.md` | NEW — this spec |
| `docs/superpowers/retros/_template.md` | NEW — retro template |
| `docs/superpowers/retros/2026-06-retrospective.md` | NEW — June 2026 retro |
| `.trae/rules/project_rules.md` | Add ai-context reference |

## Verification

After implementation:
1. Open a fresh AI session, ask "What is this repo?" → AI should cite `ai-context.md` and quote its service map
2. Verify all 8 services appear in the service map table
3. Verify all 5 phases of work (P1.1 → P3.3) appear in retrospective