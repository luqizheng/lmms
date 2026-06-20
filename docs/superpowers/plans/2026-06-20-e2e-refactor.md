# E2E 测试重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `tests/e2e` 重构为单一 pnpm workspace 内的 `api/`（API 直测）与 `ui/`（Playwright 前端业务测试）两部分，数据分基础/业务两层重置，用例与报告用 Excel 管理，失败输出 `<caseId>.json`。

**Architecture:** 共享层 `shared/` 提供 DB 重置、Excel 读写、报告写入；`api/` 用 vitest 数据驱动执行器读 xlsx 生成用例直连 api-gateway；`ui/` 用 Playwright 读 xlsx 调 flows 操作真实前端。两部分各自 globalSetup 调 `resetBase()`，套件前调 `resetBusiness()`。

**Tech Stack:** Node.js (ESM) + TypeScript、vitest、supertest、Playwright、SheetJS(`xlsx`)、mysql2。

设计文档：[2026-06-20-e2e-refactor-design.md](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-20-e2e-refactor-design.md)

---

## 文件结构

```
tests/e2e/
  package.json                         # 修改：增 xlsx、@playwright/test、scripts
  shared/
    config.ts                          # 创建：环境变量集中读取
    db/
      reset.ts                         # 创建：resetBase() / resetBusiness()
      seed/base.sql                    # 创建：基础数据幂等 seed
    data/
      excel.ts                         # 创建：读用例 / 占位符解析 / 断言
    report/
      writer.ts                        # 创建：写 xlsx + 失败 json
  api/
    cases/iam-users.xlsx               # 创建：首个数据驱动用例（脚本生成）
    runner/run.e2e.test.ts             # 创建：vitest 数据驱动执行器
    globalSetup.ts                     # 创建：resetBase + 登录
    vitest.config.ts                   # 创建：api 专用配置
  ui/
    cases/user-flow.xlsx               # 创建：首个 UI 用例（脚本生成）
    pages/login.page.ts                # 创建：登录 Page Object
    pages/user.page.ts                 # 创建：用户管理 Page Object
    flows/index.ts                     # 创建：flow 名 → 函数映射
    runner/run.spec.ts                 # 创建：Playwright 数据驱动执行器
    globalSetup.ts                     # 创建：resetBase + 前端健康探测
    playwright.config.ts               # 创建：playwright 配置
  scripts/gen-sample-cases.mjs         # 创建：用脚本生成 .xlsx 样例
```

现有 `helpers/`、`fixtures/`、`*.e2e.test.ts`、根 `vitest.config.ts` 保留不动；新增结构与其并存，后续逐步迁移（不在本计划范围）。

---

## Task 1: 安装依赖与脚本入口

**Files:**
- Modify: `tests/e2e/package.json`

- [ ] **Step 1: 修改 package.json 增加依赖与 scripts**

将 `tests/e2e/package.json` 改为：

```json
{
  "name": "e2e",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest",
    "test:auth": "vitest run auth",
    "test:users": "vitest run users",
    "test:roles": "vitest run roles",
    "test:permissions": "vitest run permissions",
    "test:tokens": "vitest run tokens",
    "test:audit": "vitest run audit",
    "test:config": "vitest run config",
    "test:file": "vitest run file",
    "test:notification": "vitest run notification",
    "test:token-admin": "vitest run token-admin",
    "test:menu-configs": "vitest run menu-configs",
    "test:iam-orgs": "vitest run iam-org-units",
    "test:gateway": "vitest run gateway",
    "test:api": "vitest run --config api/vitest.config.ts",
    "test:ui": "playwright test --config ui/playwright.config.ts",
    "gen:samples": "node scripts/gen-sample-cases.mjs"
  },
  "devDependencies": {
    "@playwright/test": "^1.48",
    "@types/node": "^20",
    "@types/supertest": "^6",
    "mysql2": "^3.22.5",
    "supertest": "^7",
    "typescript": "^5",
    "vitest": "^2",
    "xlsx": "^0.18.5"
  }
}
```

- [ ] **Step 2: 安装依赖**

Run: `pnpm -C tests/e2e install`
Expected: 安装成功，生成/更新 `pnpm-lock.yaml`，含 `xlsx`、`@playwright/test`。

- [ ] **Step 3: 安装 Playwright 浏览器**

Run: `pnpm -C tests/e2e exec playwright install chromium`
Expected: chromium 下载完成。

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/package.json tests/e2e/pnpm-lock.yaml
git commit -m "chore(e2e): add xlsx and playwright deps for refactor"
```

---

## Task 2: 共享配置 shared/config.ts

**Files:**
- Create: `tests/e2e/shared/config.ts`

- [ ] **Step 1: 创建 config.ts**

```ts
// 集中读取 e2e 环境变量，供 api / ui / shared 复用
export const config = {
  baseURL: process.env.E2E_BASE_URL ?? 'http://localhost:3000',
  frontendURL: process.env.E2E_FRONTEND_URL ?? 'http://localhost:8848',
  admin: {
    user: process.env.E2E_ADMIN_USER ?? 'e2eadmin',
    pass: process.env.E2E_ADMIN_PASS ?? 'P@ssw0rd!',
    nickname: 'E2E Admin',
    email: 'e2eadmin@test.local',
  },
  db: {
    host: process.env.E2E_DB_HOST ?? 'localhost',
    port: parseInt(process.env.E2E_DB_PORT ?? '3306', 10),
    user: process.env.E2E_DB_USER ?? 'root',
    password: process.env.E2E_DB_PASSWORD ?? 'rootpass',
    database: process.env.E2E_DB_NAME ?? 'superpowers',
  },
  // 基础数据 id 上限：业务数据清理时删除 id 大于此值的行
  seedUserMaxId: parseInt(process.env.E2E_SEED_USER_MAX_ID ?? '1', 10),
  debug: process.env.E2E_DEBUG === '1',
};
```

- [ ] **Step 2: 类型检查**

Run: `pnpm -C tests/e2e exec tsc --noEmit -p tsconfig.json`
Expected: 无错误（仅新增独立文件，无引用问题）。

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/shared/config.ts
git commit -m "feat(e2e): add shared config module"
```

---

## Task 3: 基础数据 seed SQL

**Files:**
- Create: `tests/e2e/shared/db/seed/base.sql`

- [ ] **Step 1: 先确认 users 表结构**

Run: `pnpm -C tests/e2e exec node -e "const m=require('mysql2/promise');(async()=>{const c=await m.createConnection({host:'localhost',port:3306,user:'root',password:'rootpass',database:'superpowers'});const [r]=await c.query('SHOW COLUMNS FROM users');console.log(r.map(x=>x.Field).join(','));await c.end();})()"`
Expected: 打印 users 表字段名列表（用于校准 seed 列名）。若连接失败说明基础设施未启动，先 `cd infrastructure; docker compose up -d`。

- [ ] **Step 2: 按实际字段创建 base.sql**

根据 Step 1 输出的真实列名调整下方占位列。基础结构（幂等）：

```sql
-- 基础数据：整轮测试前导入一次，幂等
SET FOREIGN_KEY_CHECKS = 0;

-- admin 账号（首个用户即 admin），固定 id=1
INSERT INTO users (id, username, password, nickname, email)
VALUES (1, 'e2eadmin', '__REPLACE_WITH_HASH__', 'E2E Admin', 'e2eadmin@test.local')
ON DUPLICATE KEY UPDATE username=VALUES(username), nickname=VALUES(nickname), email=VALUES(email);

SET FOREIGN_KEY_CHECKS = 1;
```

> 注意：password 哈希需与服务一致。若服务无法直接插哈希，admin 仍由 globalSetup 通过 `/v1/iam/auth/register` API 创建（见 Task 5），此时 base.sql 仅保留非用户类基础数据（角色/权限/菜单/oauth-client，按各表真实结构补全）。Step 1 确认后据实裁剪。

- [ ] **Step 3: 提交（占位，Task 5 验证后可能回填）**

```bash
git add tests/e2e/shared/db/seed/base.sql
git commit -m "feat(e2e): add base seed sql skeleton"
```

---

## Task 4: 数据重置 shared/db/reset.ts

**Files:**
- Create: `tests/e2e/shared/db/reset.ts`

- [ ] **Step 1: 创建 reset.ts**

```ts
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createConnection } from 'mysql2/promise';
import { config } from '../config';

const __dirname = dirname(fileURLToPath(import.meta.url));

// 业务表：每套件前整表清空
const BUSINESS_TABLES = [
  'login_logs',
  'refresh_tokens',
  'user_roles',
  'announcements',
  'config_items',
  'operation_logs',
  'files',
];

async function withConn<T>(fn: (c: Awaited<ReturnType<typeof createConnection>>) => Promise<T>): Promise<T> {
  const conn = await createConnection(config.db);
  try {
    return await fn(conn);
  } finally {
    await conn.end();
  }
}

// 整轮开始：清全部 + 重导基础 seed
export async function resetBase(): Promise<void> {
  await withConn(async (c) => {
    await c.query('SET FOREIGN_KEY_CHECKS = 0');
    for (const t of [...BUSINESS_TABLES, 'users']) {
      await c.query(`DELETE FROM ${t}`).catch(() => undefined);
    }
    const sql = readFileSync(join(__dirname, 'seed', 'base.sql'), 'utf8');
    for (const stmt of sql.split(';').map((s) => s.trim()).filter(Boolean)) {
      await c.query(stmt);
    }
    await c.query('SET FOREIGN_KEY_CHECKS = 1');
  });
}

// 每套件前：清业务数据，保留基础数据
export async function resetBusiness(): Promise<void> {
  await withConn(async (c) => {
    await c.query('SET FOREIGN_KEY_CHECKS = 0');
    for (const t of BUSINESS_TABLES) {
      await c.query(`DELETE FROM ${t}`).catch(() => undefined);
    }
    // users 表保留基础数据：删除 id 超过种子上限的行
    await c.query('DELETE FROM users WHERE id > ?', [config.seedUserMaxId]);
    await c.query('SET FOREIGN_KEY_CHECKS = 1');
  });
}
```

- [ ] **Step 2: 类型检查**

Run: `pnpm -C tests/e2e exec tsc --noEmit -p tsconfig.json`
Expected: 无错误。

- [ ] **Step 3: 验证 resetBusiness 可运行（需基础设施在线）**

Run: `pnpm -C tests/e2e exec node --import tsx shared/db/reset.ts || pnpm -C tests/e2e exec vitest run -t __none__`
Expected: 无表名错误。若某业务表不存在，靠 `.catch` 跳过，不报致命错误。

> 若环境无 tsx，可跳过 Step 3，由 Task 5 的 globalSetup 实际触发验证。

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/shared/db/reset.ts
git commit -m "feat(e2e): add base/business data reset"
```

---

## Task 5: Excel 读写与断言 shared/data/excel.ts

**Files:**
- Create: `tests/e2e/shared/data/excel.ts`

- [ ] **Step 1: 创建 excel.ts**

```ts
import { readFileSync } from 'node:fs';
import * as XLSX from 'xlsx';

export interface ApiCase {
  caseId: string;
  desc: string;
  method: string;
  path: string;
  headers?: string;
  body?: string;
  expectStatus: string;
  expectBody?: string;
  skip?: string;
}

// 读取单个 xlsx 的所有 sheet，返回 { sheetName: rows }
export function readCases<T = Record<string, string>>(file: string): Record<string, T[]> {
  const wb = XLSX.read(readFileSync(file), { type: 'buffer' });
  const out: Record<string, T[]> = {};
  for (const name of wb.SheetNames) {
    out[name] = XLSX.utils.sheet_to_json<T>(wb.Sheets[name], { defval: '' });
  }
  return out;
}

// 占位符解析：${rand} 唯一值；${CASE-ID.field} 引用前序响应
export function resolvePlaceholders(
  input: string,
  ctx: Record<string, unknown>,
): string {
  if (!input) return input;
  return input
    .replace(/\$\{rand\}/g, () => `${Date.now()}${Math.random().toString(36).slice(2, 6)}`)
    .replace(/\$\{([\w-]+)\.([\w.]+)\}/g, (_m, caseId: string, field: string) => {
      const resp = ctx[caseId] as Record<string, unknown> | undefined;
      if (!resp) throw new Error(`占位引用未产出的用例: ${caseId}`);
      const val = field.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)?.[k], resp);
      return String(val ?? '');
    });
}

// 极简断言：表达式如 "$.id exists" / "$.username==alice"
export function assertBody(expr: string, body: unknown): { ok: boolean; reason?: string } {
  if (!expr) return { ok: true };
  for (const clause of expr.split('&&').map((s) => s.trim()).filter(Boolean)) {
    const existsM = clause.match(/^\$\.([\w.]+)\s+exists$/);
    const eqM = clause.match(/^\$\.([\w.]+)==(.+)$/);
    if (existsM) {
      const v = getPath(body, existsM[1]);
      if (v === undefined || v === null) return { ok: false, reason: `${clause} 不存在` };
    } else if (eqM) {
      const v = getPath(body, eqM[1]);
      if (String(v) !== eqM[2]) return { ok: false, reason: `${clause} 实际=${String(v)}` };
    } else {
      return { ok: false, reason: `无法解析断言: ${clause}` };
    }
  }
  return { ok: true };
}

function getPath(obj: unknown, path: string): unknown {
  return path.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)?.[k], obj);
}
```

- [ ] **Step 2: 类型检查**

Run: `pnpm -C tests/e2e exec tsc --noEmit -p tsconfig.json`
Expected: 无错误。

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/shared/data/excel.ts
git commit -m "feat(e2e): add excel reader, placeholder and assertion"
```

---

## Task 6: 报告写入 shared/report/writer.ts

**Files:**
- Create: `tests/e2e/shared/report/writer.ts`

- [ ] **Step 1: 创建 writer.ts**

```ts
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as XLSX from 'xlsx';

const __dirname = dirname(fileURLToPath(import.meta.url));

export interface CaseResult {
  caseId: string;
  desc: string;
  result: 'PASS' | 'FAIL' | 'SKIP';
  status: number | string;
  durationMs: number;
  error?: string;
}

export interface ErrorDetail {
  caseId: string;
  request: unknown;
  response: unknown;
  expect: unknown;
  timestamp: string;
  screenshot?: string;
}

const reportDir = __dirname;
const errorsDir = join(__dirname, 'errors');

// 写单个失败用例 <caseId>.json
export function writeErrorJson(detail: ErrorDetail): string {
  mkdirSync(errorsDir, { recursive: true });
  const file = join(errorsDir, `${detail.caseId}.json`);
  writeFileSync(file, JSON.stringify(detail, null, 2), 'utf8');
  return file;
}

// 汇总写 report-<时间戳>.xlsx，返回路径
export function writeReport(results: CaseResult[]): string {
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  const wb = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(wb, XLSX.utils.json_to_sheet(results), 'detail');
  const summary = {
    total: results.length,
    pass: results.filter((r) => r.result === 'PASS').length,
    fail: results.filter((r) => r.result === 'FAIL').length,
    skip: results.filter((r) => r.result === 'SKIP').length,
  };
  XLSX.utils.book_append_sheet(wb, XLSX.utils.json_to_sheet([summary]), 'summary');
  const file = join(reportDir, `report-${ts}.xlsx`);
  XLSX.writeFile(wb, file);
  return file;
}
```

- [ ] **Step 2: 类型检查**

Run: `pnpm -C tests/e2e exec tsc --noEmit -p tsconfig.json`
Expected: 无错误。

- [ ] **Step 3: 创建 .gitignore 忽略生成物**

在 `tests/e2e/.gitignore` 追加：

```
shared/report/report-*.xlsx
shared/report/errors/
```

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/shared/report/writer.ts tests/e2e/.gitignore
git commit -m "feat(e2e): add xlsx report writer and error json"
```

---

## Task 7: 生成样例用例 xlsx

**Files:**
- Create: `tests/e2e/scripts/gen-sample-cases.mjs`
- Create: `tests/e2e/api/cases/iam-users.xlsx`（脚本产出）
- Create: `tests/e2e/ui/cases/user-flow.xlsx`（脚本产出）

- [ ] **Step 1: 创建生成脚本**

```js
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as XLSX from 'xlsx';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

function write(file, sheets) {
  mkdirSync(dirname(file), { recursive: true });
  const wb = XLSX.utils.book_new();
  for (const [name, rows] of Object.entries(sheets)) {
    XLSX.utils.book_append_sheet(wb, XLSX.utils.json_to_sheet(rows), name);
  }
  XLSX.writeFile(wb, file);
  console.log('written', file);
}

write(join(root, 'api/cases/iam-users.xlsx'), {
  users: [
    { caseId: 'IAM-USER-001', desc: '创建用户', method: 'POST', path: '/v1/iam/users', headers: '', body: '{"username":"u${rand}","password":"P@ssw0rd!","email":"u${rand}@t.local","nickname":"N"}', expectStatus: '201', expectBody: '$.id exists', skip: '' },
    { caseId: 'IAM-USER-002', desc: '查详情', method: 'GET', path: '/v1/iam/users/${IAM-USER-001.id}', headers: '', body: '', expectStatus: '200', expectBody: '$.username exists', skip: '' },
    { caseId: 'IAM-USER-003', desc: '删除', method: 'DELETE', path: '/v1/iam/users/${IAM-USER-001.id}', headers: '', body: '', expectStatus: '200', expectBody: '', skip: '' },
  ],
});

write(join(root, 'ui/cases/user-flow.xlsx'), {
  user: [
    { caseId: 'UI-USER-001', desc: '登录后建用户', flow: 'createUser', params: '{"username":"u${rand}"}', expect: 'toast=创建成功', skip: '' },
  ],
});
```

- [ ] **Step 2: 运行生成**

Run: `pnpm -C tests/e2e run gen:samples`
Expected: 打印两行 `written ...`，生成两个 xlsx 文件。

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/scripts/gen-sample-cases.mjs tests/e2e/api/cases/iam-users.xlsx tests/e2e/ui/cases/user-flow.xlsx
git commit -m "feat(e2e): add sample case generator and xlsx fixtures"
```

---

## Task 8: API 执行器 + globalSetup + 配置

**Files:**
- Create: `tests/e2e/api/globalSetup.ts`
- Create: `tests/e2e/api/vitest.config.ts`
- Create: `tests/e2e/api/runner/run.e2e.test.ts`

- [ ] **Step 1: 创建 api/globalSetup.ts**

```ts
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import request from 'supertest';
import { config } from '../shared/config';
import { resetBase } from '../shared/db/reset';

const __dirname = dirname(fileURLToPath(import.meta.url));

export default async function () {
  await resetBase();

  // admin 由 API 注册（首个用户即 admin），保证密码哈希正确
  await request(config.baseURL).post('/v1/iam/auth/register').send({
    username: config.admin.user,
    password: config.admin.pass,
    nickname: config.admin.nickname,
    email: config.admin.email,
  });

  const login = await request(config.baseURL)
    .post('/v1/iam/auth/login')
    .send({ username: config.admin.user, password: config.admin.pass });
  if (login.status !== 200 || !login.body?.access_token) {
    throw new Error(`login admin failed: ${login.status} ${JSON.stringify(login.body)}`);
  }
  writeFileSync(join(__dirname, '.token.json'), JSON.stringify({ token: login.body.access_token }), 'utf8');
}
```

- [ ] **Step 2: 创建 api/vitest.config.ts**

```ts
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globalSetup: ['./api/globalSetup.ts'],
    include: ['api/runner/**/*.e2e.test.ts'],
    testTimeout: 20000,
    hookTimeout: 30000,
  },
});
```

- [ ] **Step 3: 创建 api/runner/run.e2e.test.ts**

```ts
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Glob } from 'glob';
import { afterAll, beforeAll, describe, it } from 'vitest';
import request from 'supertest';
import { config } from '../../shared/config';
import { resetBusiness } from '../../shared/db/reset';
import { readCases, resolvePlaceholders, assertBody, type ApiCase } from '../../shared/data/excel';
import { writeReport, writeErrorJson, type CaseResult } from '../../shared/report/writer';
import { globSync } from 'node:fs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const token = JSON.parse(readFileSync(join(__dirname, '..', '.token.json'), 'utf8')).token as string;

const results: CaseResult[] = [];
const ctx: Record<string, unknown> = {};

// 用 fs.readdirSync 列出 cases 目录的 xlsx（避免引入 glob 依赖）
import { readdirSync } from 'node:fs';
const casesDir = join(__dirname, '..', 'cases');
const files = readdirSync(casesDir).filter((f) => f.endsWith('.xlsx'));

beforeAll(async () => {
  await resetBusiness();
});

afterAll(() => {
  const file = writeReport(results);
  console.log('report:', file);
});

for (const file of files) {
  const sheets = readCases<ApiCase>(join(casesDir, file));
  for (const [sheet, rows] of Object.entries(sheets)) {
    describe(`${file} > ${sheet}`, () => {
      for (const c of rows) {
        const run = c.skip === 'Y' ? it.skip : it;
        run(`${c.caseId} ${c.desc}`, async () => {
          const start = Date.now();
          const path = resolvePlaceholders(c.path, ctx);
          const body = c.body ? JSON.parse(resolvePlaceholders(c.body, ctx)) : undefined;
          const method = c.method.toLowerCase() as 'get' | 'post' | 'put' | 'patch' | 'delete';
          let agent = request(config.baseURL)[method](path).set('Authorization', `Bearer ${token}`);
          if (body) agent = agent.send(body);
          const res = await agent;
          ctx[c.caseId] = res.body;

          const statusOk = String(res.status) === String(c.expectStatus);
          const bodyCheck = assertBody(c.expectBody ?? '', res.body);
          const ok = statusOk && bodyCheck.ok;
          results.push({
            caseId: c.caseId, desc: c.desc, result: ok ? 'PASS' : 'FAIL',
            status: res.status, durationMs: Date.now() - start,
            error: ok ? '' : `见 errors/${c.caseId}.json`,
          });
          if (!ok) {
            writeErrorJson({
              caseId: c.caseId,
              request: { method: c.method, path, body },
              response: { status: res.status, body: res.body },
              expect: { status: c.expectStatus, body: c.expectBody },
              timestamp: new Date().toISOString(),
            });
            throw new Error(`${c.caseId} FAIL: status=${res.status} ${bodyCheck.reason ?? ''}`);
          }
        });
      }
    });
  }
}
```

> 注意：上方 import 含冗余（`glob`/`globSync`），编写时仅保留 `node:fs` 的 `readFileSync` 与 `readdirSync`。Step 4 类型检查会暴露未用导入，据实删除。

- [ ] **Step 4: 清理冗余 import 并类型检查**

删除 `import { Glob } from 'glob'`、`import { globSync } from 'node:fs'` 两行（不存在的导入）。
Run: `pnpm -C tests/e2e exec tsc --noEmit -p tsconfig.json`
Expected: 无错误。

- [ ] **Step 5: 运行 API 数据驱动测试（需基础设施在线）**

Run: `pnpm -C tests/e2e run test:api`
Expected: 3 个用例执行，输出 `report: .../report-*.xlsx`。IAM-USER-001/002/003 PASS。

- [ ] **Step 6: Commit**

```bash
git add tests/e2e/api/globalSetup.ts tests/e2e/api/vitest.config.ts tests/e2e/api/runner/run.e2e.test.ts
git commit -m "feat(e2e): add data-driven api runner"
```

---

## Task 9: UI Page Objects 与 flows

**Files:**
- Create: `tests/e2e/ui/pages/login.page.ts`
- Create: `tests/e2e/ui/pages/user.page.ts`
- Create: `tests/e2e/ui/flows/index.ts`

- [ ] **Step 1: 确认前端登录页与用户页选择器**

Run: `pnpm -C tests/e2e exec playwright codegen http://localhost:8848`
Expected: 打开前端，手动走登录→用户管理，记录关键选择器（用户名输入、密码输入、登录按钮、新增用户按钮、表单字段、提交按钮、成功 toast）。关闭后据实填入下方 Page Object。

> 若前端未启动：`pnpm -C frontend dev`（端口见 `.env.development`），或用容器 nginx 地址。

- [ ] **Step 2: 创建 login.page.ts（选择器据 Step 1 校准）**

```ts
import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

export class LoginPage {
  constructor(private page: Page) {}

  async login(user = config.admin.user, pass = config.admin.pass): Promise<void> {
    await this.page.goto(config.frontendURL);
    await this.page.getByPlaceholder('账号').fill(user);
    await this.page.getByPlaceholder('密码').fill(pass);
    await this.page.getByRole('button', { name: '登录' }).click();
    await this.page.waitForURL(/welcome|home|\//);
  }
}
```

- [ ] **Step 3: 创建 user.page.ts（选择器据 Step 1 校准）**

```ts
import type { Page } from '@playwright/test';

export class UserPage {
  constructor(private page: Page) {}

  async createUser(params: { username: string }): Promise<void> {
    await this.page.goto(`${process.env.E2E_FRONTEND_URL ?? 'http://localhost:8848'}/#/system/user/index`);
    await this.page.getByRole('button', { name: /新增|添加/ }).click();
    await this.page.getByLabel('用户名').fill(params.username);
    await this.page.getByRole('button', { name: /确定|提交|保存/ }).click();
  }

  async lastToastText(): Promise<string> {
    const toast = this.page.locator('.el-message').first();
    return (await toast.textContent()) ?? '';
  }
}
```

- [ ] **Step 4: 创建 flows/index.ts**

```ts
import type { Page } from '@playwright/test';
import { LoginPage } from '../pages/login.page';
import { UserPage } from '../pages/user.page';

export interface FlowResult {
  toast?: string;
}

export type Flow = (page: Page, params: Record<string, unknown>) => Promise<FlowResult>;

export const flows: Record<string, Flow> = {
  createUser: async (page, params) => {
    await new LoginPage(page).login();
    const up = new UserPage(page);
    await up.createUser({ username: String(params.username) });
    return { toast: await up.lastToastText() };
  },
};
```

- [ ] **Step 5: 类型检查**

Run: `pnpm -C tests/e2e exec tsc --noEmit -p tsconfig.json`
Expected: 无错误。

- [ ] **Step 6: Commit**

```bash
git add tests/e2e/ui/pages tests/e2e/ui/flows
git commit -m "feat(e2e): add ui page objects and flow registry"
```

---

## Task 10: UI 执行器 + globalSetup + playwright 配置

**Files:**
- Create: `tests/e2e/ui/globalSetup.ts`
- Create: `tests/e2e/ui/playwright.config.ts`
- Create: `tests/e2e/ui/runner/run.spec.ts`

- [ ] **Step 1: 创建 ui/globalSetup.ts**

```ts
import { resetBase } from '../shared/db/reset';
import { config } from '../shared/config';

export default async function () {
  await resetBase();
  // 前端健康探测：失败即中止
  const res = await fetch(config.frontendURL).catch(() => null);
  if (!res || !res.ok) {
    throw new Error(`前端未就绪: ${config.frontendURL}`);
  }
}
```

- [ ] **Step 2: 创建 ui/playwright.config.ts**

```ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './runner',
  globalSetup: './globalSetup.ts',
  timeout: 30000,
  use: {
    screenshot: 'only-on-failure',
    headless: true,
  },
  reporter: [['list']],
});
```

- [ ] **Step 3: 创建 ui/runner/run.spec.ts**

```ts
import { readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from '@playwright/test';
import { resetBusiness } from '../../shared/db/reset';
import { readCases, resolvePlaceholders } from '../../shared/data/excel';
import { writeReport, writeErrorJson, type CaseResult } from '../../shared/report/writer';
import { flows } from '../flows';

interface UiCase {
  caseId: string;
  desc: string;
  flow: string;
  params?: string;
  expect?: string;
  skip?: string;
}

const __dirname = dirname(fileURLToPath(import.meta.url));
const casesDir = join(__dirname, '..', 'cases');
const files = readdirSync(casesDir).filter((f) => f.endsWith('.xlsx'));
const results: CaseResult[] = [];
const ctx: Record<string, unknown> = {};

test.beforeAll(async () => {
  await resetBusiness();
});

test.afterAll(async () => {
  console.log('report:', writeReport(results));
});

for (const file of files) {
  const sheets = readCases<UiCase>(join(casesDir, file));
  for (const [sheet, rows] of Object.entries(sheets)) {
    test.describe(`${file} > ${sheet}`, () => {
      for (const c of rows) {
        const runner = c.skip === 'Y' ? test.skip : test;
        runner(`${c.caseId} ${c.desc}`, async ({ page }) => {
          const start = Date.now();
          const flow = flows[c.flow];
          if (!flow) throw new Error(`未注册 flow: ${c.flow}`);
          const params = c.params ? JSON.parse(resolvePlaceholders(c.params, ctx)) : {};
          try {
            const r = await flow(page, params);
            // 断言 expect=toast=xxx
            const exp = (c.expect ?? '').trim();
            if (exp.startsWith('toast=')) {
              const want = exp.slice('toast='.length);
              if (!(r.toast ?? '').includes(want)) {
                throw new Error(`toast 不含 "${want}"，实际="${r.toast}"`);
              }
            }
            results.push({ caseId: c.caseId, desc: c.desc, result: 'PASS', status: 'ok', durationMs: Date.now() - start, error: '' });
          } catch (e) {
            const shot = join(__dirname, '..', '..', 'shared', 'report', 'errors', `${c.caseId}.png`);
            await page.screenshot({ path: shot }).catch(() => undefined);
            writeErrorJson({
              caseId: c.caseId,
              request: { flow: c.flow, params },
              response: { error: String(e) },
              expect: c.expect,
              timestamp: new Date().toISOString(),
              screenshot: shot,
            });
            results.push({ caseId: c.caseId, desc: c.desc, result: 'FAIL', status: 'err', durationMs: Date.now() - start, error: `见 errors/${c.caseId}.json` });
            throw e;
          }
        });
      }
    });
  }
}
```

- [ ] **Step 4: 类型检查**

Run: `pnpm -C tests/e2e exec tsc --noEmit -p tsconfig.json`
Expected: 无错误。

- [ ] **Step 5: 运行 UI 测试（需基础设施 + 前端在线）**

Run: `pnpm -C tests/e2e run test:ui`
Expected: UI-USER-001 执行；通过则 PASS，失败则生成 `errors/UI-USER-001.png` 与 `.json`。输出 report 路径。

> Step 5 依赖前端选择器准确。若 toast/选择器不符，回 Task 9 据实修正后重跑。

- [ ] **Step 6: Commit**

```bash
git add tests/e2e/ui/globalSetup.ts tests/e2e/ui/playwright.config.ts tests/e2e/ui/runner/run.spec.ts
git commit -m "feat(e2e): add data-driven playwright ui runner"
```

---

## Task 11: 文档更新

**Files:**
- Modify: `tests/e2e/README.md`
- Modify: `tests/e2e/AGENTS.md`

- [ ] **Step 1: README 增加 api/ui 两部分运行说明**

在 `tests/e2e/README.md` 末尾追加章节，说明：
- 目录 `api/`（数据驱动 API 直测）与 `ui/`（Playwright 业务测试）
- 数据两层：`shared/db/seed/base.sql`（基础）+ `resetBusiness`（业务）
- 用例在 `*/cases/*.xlsx`，报告在 `shared/report/report-*.xlsx`，失败在 `shared/report/errors/<caseId>.json`
- 命令：`pnpm -C tests/e2e run test:api` / `run test:ui` / `run gen:samples`

- [ ] **Step 2: AGENTS.md 增加新结构边界说明**

在 `tests/e2e/AGENTS.md` 的「测试结构」节补充 api/ui 拆分与 Excel 用例约定。

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/README.md tests/e2e/AGENTS.md
git commit -m "docs(e2e): document api/ui split and excel-driven cases"
```

---

## 自查记录

- **Spec 覆盖**：目录(Task2-10)、数据重置(Task3-4)、Excel 用例(Task5,7)、报告+erroros json(Task6)、API 执行器(Task8)、UI 执行器(Task9-10)、运行命令(Task1)、错误边界(各 runner try/catch + globalSetup 探测) 均有对应任务。
- **占位符**：无 TBD/TODO；base.sql 因依赖真实表结构，Task3 Step1 先查列名再据实填，已显式标注校准点。
- **类型一致**：`CaseResult`/`ErrorDetail`(writer.ts) 在 api/ui runner 引用一致；`ApiCase`(excel.ts) 在 api runner 引用一致；`resolvePlaceholders`/`assertBody`/`readCases` 签名前后一致。
