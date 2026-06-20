import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, it } from 'vitest';
import request from 'supertest';
import { config } from '../../shared/config';
import { resetBusiness } from '../../shared/db/reset';
import { readCases, resolve, assertBody, type ApiCase } from '../../shared/data/cases';
import {
  writeReport,
  writeErrorJson,
  type CaseResult,
} from '../../shared/report/writer';

const __dirname = dirname(fileURLToPath(import.meta.url));
const token = JSON.parse(
  readFileSync(join(__dirname, '..', '.token.json'), 'utf8'),
).token as string;

const results: CaseResult[] = [];
const ctx: Record<string, unknown> = {};

const casesDir = join(__dirname, '..', 'cases');
const files = readdirSync(casesDir).filter((f) => /\.ya?ml$/.test(f));

beforeAll(async () => {
  await resetBusiness();
});

afterAll(() => {
  const file = writeReport(results);
  console.log('report:', file);
});

for (const file of files) {
  const { group, cases } = readCases<ApiCase>(join(casesDir, file));
  describe(`${file} > ${group}`, () => {
    for (const c of cases) {
      const run = c.skip ? it.skip : it;
      run(`${c.caseId} ${c.desc}`, async () => {
        const start = Date.now();
        const path = resolve(c.path, ctx);
        const body = c.body !== undefined ? resolve(c.body, ctx) : undefined;
        const headers = c.headers ? resolve(c.headers, ctx) : undefined;
        const method = c.method.toLowerCase() as
          | 'get'
          | 'post'
          | 'put'
          | 'patch'
          | 'delete';
        let agent = request(config.baseURL)
          [method](path)
          .set('Authorization', `Bearer ${token}`);
        if (headers) {
          for (const [k, v] of Object.entries(headers)) agent = agent.set(k, v);
        }
        if (body !== undefined) agent = agent.send(body as object);
        const res = await agent;
        ctx[c.caseId] = res.body;

        const statusOk = String(res.status) === String(c.expectStatus);
        const bodyCheck = assertBody(c.expect, res.body);
        const ok = statusOk && bodyCheck.ok;
        results.push({
          caseId: c.caseId,
          desc: c.desc,
          result: ok ? 'PASS' : 'FAIL',
          status: res.status,
          durationMs: Date.now() - start,
          error: ok ? '' : `见 errors/${c.caseId}.json`,
        });
        if (!ok) {
          writeErrorJson({
            caseId: c.caseId,
            request: { method: c.method, path, body },
            response: { status: res.status, body: res.body },
            expect: { status: c.expectStatus, body: c.expect },
            timestamp: new Date().toISOString(),
          });
          throw new Error(
            `${c.caseId} FAIL: status=${res.status} (期望${c.expectStatus}) ${bodyCheck.reason ?? ''}`,
          );
        }
      });
    }
  });
}
