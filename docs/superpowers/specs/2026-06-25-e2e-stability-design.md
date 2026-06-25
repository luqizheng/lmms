# P3.3 E2E Stability Design

## Context

Current e2e is fully data-driven (per `2026-06-20-e2e-refactor-design.md`) but stability is uneven:
- No retry policy → flaky tests (e.g. WS push, async cleanup) fail the whole tier
- No pre-flight → tests start even when infra is half-up, producing cascading failures
- No smoke tier → developers wait 15+ minutes for full API+UI to validate a small change
- No tier-aware timeout → network-bound cases share a single 20s timeout with DB-heavy cases

## Decision

### 1. Smoke subset via `smoke: true` flag in cases

YAML schema gains an optional field:

```yaml
cases:
  - caseId: IAM-USER-001
    smoke: true   # ← new
    ...
```

Runner filters:
- `test:smoke` → run only `smoke: true` cases
- `test:api` / `test:ui` → run all cases (default)

### 2. Retry policy

Vitest config:
```ts
test: {
  retry: process.env.E2E_RETRY ? Number(process.env.E2E_RETRY) : 1,
  ...
}
```

Playwright config:
```ts
retries: process.env.CI ? 2 : 0,
```

Only retry on network errors, not on assertion failures. (`test.retry` already retries all failures — acceptable trade-off for v1; assertion-failure retry is bounded by the assertions themselves.)

### 3. Pre-flight health check

`tests/e2e/shared/health.ts`:
```ts
export async function preflight(): Promise<void> {
  const targets = [
    `${config.baseURL}/health`,   // api-gateway
    `${config.baseURL}/iam/health`, // iam via gateway
  ];
  for (const url of targets) {
    const res = await fetch(url, { signal: AbortSignal.timeout(3000) });
    if (!res.ok) throw new Error(`preflight fail: ${url}`);
  }
}
```

Called from both `globalSetup.ts` files (api + ui) BEFORE `resetBase()`.

### 4. Tier-based scripts

`tests/e2e/package.json` adds:
```json
{
  "scripts": {
    "test:smoke": "vitest run --config api/vitest.config.ts -- --reporter=basic",
    "test:smoke:ui": "playwright test --config ui/playwright.config.ts --grep @smoke"
  }
}
```

UI cases get `@smoke` tag in addition to YAML-level flag.

### 5. Per-case timeout override

YAML schema gains optional `timeoutMs`:
```yaml
cases:
  - caseId: FILE-UPLOAD-001
    timeoutMs: 60000   # ← new (override default)
```

Runner reads and applies per case. Default = 20000ms API, 60000ms UI.

## Files Touched

| File | Change |
|------|--------|
| `tests/e2e/api/cases/*.yaml` | Add `smoke: true` to ~5 critical cases per service |
| `tests/e2e/ui/cases/*.yaml` | Add `@smoke` tag to critical flows |
| `tests/e2e/api/vitest.config.ts` | Add `retry: 1` + globalSetup preflight |
| `tests/e2e/ui/playwright.config.ts` | Add `retries: 2 (CI) / 0 (local)` |
| `tests/e2e/shared/health.ts` | NEW: preflight helper |
| `tests/e2e/api/globalSetup.ts` | Call preflight before resetBase |
| `tests/e2e/ui/globalSetup.ts` | Call preflight before resetBase |
| `tests/e2e/api/runner/run.e2e.test.ts` | Read `smoke` + `timeoutMs` flags |
| `tests/e2e/shared/data/cases.ts` | Add `smoke` + `timeoutMs` to ApiCase type |
| `tests/e2e/package.json` | Add `test:smoke` scripts |

## Verification

After implementation:
1. Mark ~5 cases per service as `smoke: true`
2. Run `pnpm -C tests/e2e test:smoke` → should complete in <2 min
3. Force a transient failure (kill redis mid-run) → verify retry kicks in
4. Stop infrastructure mid-test → verify preflight catches it cleanly