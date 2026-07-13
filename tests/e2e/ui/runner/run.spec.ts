import { readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from '@playwright/test';
import { resetBusiness } from '../../shared/db/reset';
import {
  readCases,
  resolve,
  filterBySmoke,
  type UiCase,
} from '../../shared/data/cases';
import {
  writeReport,
  writeErrorJson,
  type CaseResult,
} from '../../shared/report/writer';
import { flows } from '../flows';

const __dirname = dirname(fileURLToPath(import.meta.url));
const casesDir = join(__dirname, '..', 'cases');
const files = readdirSync(casesDir).filter((f) => /\.ya?ml$/.test(f));
const results: CaseResult[] = [];
const ctx: Record<string, unknown> = {};

const SMOKE_ONLY = process.env.E2E_SMOKE === '1';

// 一个总 describe 包裹所有用例，保证可以合法地放 test.beforeAll / test.afterAll
test.describe('e2e ui', () => {
  test.beforeAll(async () => {
    await resetBusiness();
  });

  test.afterAll(async () => {
    console.log('report:', writeReport(results));
  });

  for (const file of files) {
    const { group, cases } = readCases<UiCase>(join(casesDir, file));
    const filtered = filterBySmoke(cases, SMOKE_ONLY);
    if (filtered.length === 0) continue;
    test.describe(`${file} > ${group}`, () => {
      for (const c of filtered) {
        const runner = c.skip ? test.skip : test;
        runner(`${c.caseId} ${c.desc}`, async ({ page }) => {
          const start = Date.now();
          const flow = flows[c.flow];
          if (!flow) throw new Error(`未注册 flow: ${c.flow}`);
          const params = c.params ? resolve(c.params, ctx) : {};
          try {
            const r = await flow(page, params);
            for (const raw of c.expect ?? []) {
              const exp = raw.trim();
              if (exp.startsWith('toast=')) {
                const want = exp.slice('toast='.length);
                if (!(r.toast ?? '').includes(want)) {
                  throw new Error(`toast 不含 "${want}"，实际="${r.toast}"`);
                }
              } else if (exp.startsWith('dialog=')) {
                const want = exp.slice('dialog='.length);
                if (!(r.dialog ?? '').includes(want)) {
                  throw new Error(`dialog 不含 "${want}"，实际="${r.dialog}"`);
                }
              }
            }
            results.push({
              caseId: c.caseId,
              desc: c.desc,
              result: 'PASS',
              status: 'ok',
              durationMs: Date.now() - start,
              error: '',
            });
          } catch (e) {
            const shot = join(
              __dirname,
              '..',
              '..',
              'shared',
              'report',
              'errors',
              `${c.caseId}.png`,
            );
            await page.screenshot({ path: shot }).catch(() => undefined);
            writeErrorJson({
              caseId: c.caseId,
              request: { flow: c.flow, params },
              response: { error: String(e) },
              expect: c.expect,
              timestamp: new Date().toISOString(),
              screenshot: shot,
            });
            results.push({
              caseId: c.caseId,
              desc: c.desc,
              result: 'FAIL',
              status: 'err',
              durationMs: Date.now() - start,
              error: `见 errors/${c.caseId}.json`,
            });
            throw e;
          }
        });
      }
    });
  }
});
