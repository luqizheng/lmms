import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, it } from 'vitest';
import request from 'supertest';
import { config } from '../../shared/config';
import { resetBusiness } from '../../shared/db/reset';
import {
  readCases,
  resolve,
  assertBody,
  statusMatches,
  filterBySmoke,
  type ApiCase,
} from '../../shared/data/cases';
import {
  writeReport,
  writeErrorJson,
  type CaseResult,
} from '../../shared/report/writer';

const __dirname = dirname(fileURLToPath(import.meta.url));
const TOKEN_FILE = join(__dirname, '..', '.token.json');

const results: CaseResult[] = [];
const ctx: Record<string, unknown> = {};
let token: string | undefined;

const casesDir = join(__dirname, '..', 'cases');
const files = readdirSync(casesDir).filter((f) => /\.ya?ml$/.test(f));

const SMOKE_ONLY = process.env.E2E_SMOKE === '1';

beforeAll(async () => {
  // globalSetup 已经写好 token；若文件不存在（例如只跑了部分 suite），现场报错提示
  if (!existsSync(TOKEN_FILE)) {
    throw new Error(
      `缺失 ${TOKEN_FILE}。请确认先经过 vitest globalSetup，或重新跑 pnpm -C tests/e2e test:api。`,
    );
  }
  token = JSON.parse(readFileSync(TOKEN_FILE, 'utf8')).token as string;
  await resetBusiness();
});

afterAll(() => {
  const file = writeReport(results);
  console.log('report:', file);
});

for (const file of files) {
  const { group, cases } = readCases<ApiCase>(join(casesDir, file));
  const filtered = filterBySmoke(cases, SMOKE_ONLY);
  if (filtered.length === 0) continue;
  describe(`${file} > ${group}`, () => {
    for (const c of filtered) {
      const run = c.skip ? it.skip : it;
      run(`${c.caseId} ${c.desc}`, async () => {
        if (!token) {
          // beforeAll 失败时 vitest 也会标红，这里再补一道兜底
          throw new Error('admin token 未初始化（globalSetup 失败？）');
        }
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
        let agent = request(config.baseURL)[method](path);
        if (c.auth !== false) {
          agent = agent.set('Authorization', `Bearer ${token}`);
        }
        if (headers) {
          for (const [k, v] of Object.entries(headers)) agent = agent.set(k, v);
        }
        if (body !== undefined) agent = agent.send(body as object);
        const res = await agent;
        ctx[c.caseId] = res.body;

        const statusOk = statusMatches(res.status, c.expectStatus);
        const expectExprs = c.expect ? resolve(c.expect, ctx) : undefined;
        const bodyCheck = assertBody(expectExprs, res.body);
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
      }, c.timeoutMs ?? 20000);
    }
  });
}
