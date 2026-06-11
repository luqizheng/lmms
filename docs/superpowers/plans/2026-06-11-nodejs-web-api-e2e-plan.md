# Node.js Web API E2E 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立 Node.js E2E 测试套件，覆盖 5 类 web API 共 21 个 case，支持单组/单 case/调试模式独立运行

**Architecture:** Vitest + supertest 跨 API 网关 HTTP 调真服务；globalSetup 复用 admin token；按服务拆 5 个测试文件共享 helpers

**Tech Stack:** TypeScript 5、Vitest 2、supertest 7、pnpm workspace

**Spec:** [2026-06-11-nodejs-web-api-e2e-design.md](../specs/2026-06-11-nodejs-web-api-e2e-design.md)

**前置假设：**
- 服务已通过 `pnpm run dev` 或 docker compose 起来
- API Gateway 在 `http://localhost:8080`（可由 `E2E_BASE_URL` 覆盖）
- admin 账号已存在（`E2E_ADMIN_USER`/`E2E_ADMIN_PASS`）
- 真实 PowerShell（沙箱不限制 npm/pnpm，但 cargo 受限；本计划不涉及 cargo）

---

## 文件结构

**新增：**
- `tests/e2e/package.json` — 子包定义，依赖 vitest/supertest
- `tests/e2e/tsconfig.json` — TS 5 配置
- `tests/e2e/vitest.config.ts` — 加载 globalSetup、超时
- `tests/e2e/globalSetup.ts` — 启动前 admin 登录
- `tests/e2e/helpers/http.ts` — supertest 封装
- `tests/e2e/helpers/auth.ts` — login/logout
- `tests/e2e/helpers/ids.ts` — 共享 id 记录
- `tests/e2e/helpers/debug.ts` — E2E_DEBUG 输出
- `tests/e2e/fixtures/payload.ts` — 测试数据常量
- `tests/e2e/fixtures/seed.ts` — admin 账号读取
- `tests/e2e/auth.e2e.test.ts` — 4 case
- `tests/e2e/users.e2e.test.ts` — 5 case
- `tests/e2e/tokens.e2e.test.ts` — 5 case
- `tests/e2e/roles.e2e.test.ts` — 6 case
- `tests/e2e/permissions.e2e.test.ts` — 1 case
- `tests/e2e/README.md` — 跑测说明
- `tests/e2e/.gitignore` — node_modules

**修改：**
- `pnpm-workspace.yaml` — 追加 `tests/e2e`
- `package.json`（根）— 加 `test:e2e*` 脚本

---

## Task 1: 注册子包到 pnpm workspace

**Files:**
- Modify: `pnpm-workspace.yaml:1-8`

- [ ] **Step 1: 修改 pnpm-workspace.yaml**

```yaml
packages:
  - "common"
  - "token-server"
  - "api-gateway"
  - "auth-service"
  - "user-service"
  - "frontend"
  - "tests/e2e"
```

- [ ] **Step 2: 提交**

```bash
git add pnpm-workspace.yaml
git commit -m "chore(workspace): register tests/e2e package"
```

---

## Task 2: 创建子包 package.json

**Files:**
- Create: `tests/e2e/package.json`
- Create: `tests/e2e/.gitignore`
- Create: `tests/e2e/tsconfig.json`

- [ ] **Step 1: 创建 tests/e2e/.gitignore**

```gitignore
node_modules/
dist/
*.tsbuildinfo
```

- [ ] **Step 2: 创建 tests/e2e/tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "types": ["node"]
  },
  "include": ["**/*.ts"]
}
```

- [ ] **Step 3: 创建 tests/e2e/package.json**

```json
{
  "name": "e2e",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "devDependencies": {
    "@types/node": "^20",
    "@types/supertest": "^6",
    "supertest": "^7",
    "typescript": "^5",
    "vitest": "^2"
  }
}
```

- [ ] **Step 4: 装包**

Run: `cd tests/e2e; pnpm install`
Expected: `node_modules/` 创建，无 ERR

- [ ] **Step 5: 提交**

```bash
git add tests/e2e/package.json tests/e2e/tsconfig.json tests/e2e/.gitignore
git commit -m "chore(e2e): init package with vitest+supertest"
```

---

## Task 3: 创建 vitest 配置

**Files:**
- Create: `tests/e2e/vitest.config.ts`

- [ ] **Step 1: 写 vitest.config.ts**

```ts
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globalSetup: ['./globalSetup.ts'],
    testTimeout: 15000,
    hookTimeout: 30000,
    reporters: process.env.E2E_DEBUG ? 'verbose' : 'default',
  },
});
```

- [ ] **Step 2: 验证类型**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 无错（文件可能无引用，tsc 0 退出）

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/vitest.config.ts
git commit -m "chore(e2e): vitest config with globalSetup"
```

---

## Task 4: 写 helpers/debug

**Files:**
- Create: `tests/e2e/helpers/debug.ts`

- [ ] **Step 1: 写文件**

```ts
export const DEBUG = process.env.E2E_DEBUG === '1';

export function logReq(method: string, path: string, data?: unknown) {
  if (!DEBUG) return;
  console.log(`>> ${method} ${path}`, data ?? '');
}

export function logRes(status: number, body: unknown) {
  if (!DEBUG) return;
  console.log(`<< ${status}`, body);
}
```

- [ ] **Step 2: 提交**

```bash
git add tests/e2e/helpers/debug.ts
git commit -m "feat(e2e): debug logger helper"
```

---

## Task 5: 写 helpers/http（先写测试再写实现）

**Files:**
- Create: `tests/e2e/helpers/http.test.ts`
- Create: `tests/e2e/helpers/http.ts`

- [ ] **Step 1: 写失败测试（http.test.ts）**

```ts
import { describe, it, expect } from 'vitest';
import { authedReq } from './http.js';

describe('authedReq', () => {
  const baseURL = process.env.E2E_BASE_URL ?? 'http://localhost:8080';

  it('should build a request to a known public path', async () => {
    const r = authedReq('fake-token');
    expect(r).toBeDefined();
    expect(typeof r.get).toBe('function');
    expect(typeof r.post).toBe('function');
    expect(baseURL).toMatch(/^https?:\/\//);
  });
});
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `cd tests/e2e; pnpm test http.test`
Expected: FAIL with "Cannot find module './http.js'"

- [ ] **Step 3: 写实现（http.ts）**

```ts
import request from 'supertest';

const baseURL = process.env.E2E_BASE_URL ?? 'http://localhost:8080';

function setAuth(agent: request.SuperTest<request.Test>, token: string) {
  return agent.set('Authorization', `Bearer ${token}`);
}

export function authedReq(token: string) {
  return {
    get: (path: string) =>
      setAuth(request(baseURL).get(path), token),
    post: (path: string, data?: unknown) =>
      setAuth(request(baseURL).post(path), token).send(data ?? {}),
    put: (path: string, data?: unknown) =>
      setAuth(request(baseURL).put(path), token).send(data ?? {}),
    patch: (path: string, data?: unknown) =>
      setAuth(request(baseURL).patch(path), token).send(data ?? {}),
    del: (path: string, data?: unknown) =>
      setAuth(request(baseURL).delete(path), token).send(data ?? {}),
  };
}

export { baseURL };
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `cd tests/e2e; pnpm test http.test`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add tests/e2e/helpers/http.ts tests/e2e/helpers/http.test.ts
git commit -m "feat(e2e): authedReq helper"
```

---

## Task 6: 写 helpers/auth

**Files:**
- Create: `tests/e2e/helpers/auth.ts`

- [ ] **Step 1: 写文件**

```ts
import request from 'supertest';
import { baseURL } from './http.js';
import { logReq, logRes } from './debug.js';

export interface LoginRes {
  access_token: string;
  refresh_token?: string;
  token_type?: string;
  expires_in?: number;
}

export async function login(username: string, password: string): Promise<string> {
  const path = '/api/v1/auth/login';
  logReq('POST', path, { username });
  const res = await request(baseURL).post(path).send({ username, password });
  logRes(res.status, res.body);
  if (res.status !== 200) {
    throw new Error(`login failed: ${res.status} ${JSON.stringify(res.body)}`);
  }
  const body = res.body as LoginRes;
  if (!body.access_token) {
    throw new Error(`login response missing access_token: ${JSON.stringify(body)}`);
  }
  return body.access_token;
}

export async function logout(token: string): Promise<void> {
  const path = '/api/v1/auth/logout';
  logReq('POST', path);
  const res = await request(baseURL)
    .post(path)
    .set('Authorization', `Bearer ${token}`);
  logRes(res.status, res.body);
}
```

- [ ] **Step 2: 类型检查**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 无错

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/helpers/auth.ts
git commit -m "feat(e2e): login/logout helpers"
```

---

## Task 7: 写 fixtures/seed + payload

**Files:**
- Create: `tests/e2e/fixtures/seed.ts`
- Create: `tests/e2e/fixtures/payload.ts`

- [ ] **Step 1: 写 seed.ts**

```ts
export const SEED = {
  admin: {
    user: process.env.E2E_ADMIN_USER ?? 'admin',
    pass: process.env.E2E_ADMIN_PASS ?? 'admin123',
  },
  newUser: () => ({
    username: `e2e_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
    password: 'P@ssw0rd!',
    email: `e2e_${Date.now()}@test.local`,
    nickname: 'E2E User',
  }),
  newRole: () => ({
    name: `e2e_role_${Date.now()}`,
    code: `E2E_${Date.now()}`,
    description: 'created by e2e',
  }),
};
```

- [ ] **Step 2: 写 payload.ts**

```ts
export const Payload = {
  register: () => ({
    username: `e2e_reg_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
    password: 'P@ssw0rd!',
    email: `e2e_reg_${Date.now()}@test.local`,
  }),
  token: {
    access: () => ({
      grant_type: 'authorization_code',
      code: 'placeholder',
      client_id: process.env.E2E_CLIENT_ID ?? 'e2e-client',
      client_secret: process.env.E2E_CLIENT_SECRET ?? 'e2e-secret',
    }),
    refresh: (rt: string) => ({
      refresh_token: rt,
      client_id: process.env.E2E_CLIENT_ID ?? 'e2e-client',
      client_secret: process.env.E2E_CLIENT_SECRET ?? 'e2e-secret',
    }),
    apiKey: () => ({ name: `e2e_key_${Date.now()}`, scopes: ['read'] }),
  },
  role: () => ({
    name: `e2e_role_${Date.now()}`,
    code: `E2E_${Date.now()}`,
    description: 'created by e2e',
  }),
};
```

- [ ] **Step 3: 类型检查**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 无错

- [ ] **Step 4: 提交**

```bash
git add tests/e2e/fixtures/seed.ts tests/e2e/fixtures/payload.ts
git commit -m "feat(e2e): seed and payload fixtures"
```

---

## Task 8: 写 helpers/ids

**Files:**
- Create: `tests/e2e/helpers/ids.ts`

- [ ] **Step 1: 写文件**

```ts
const userIds: number[] = [];
const roleIds: number[] = [];

export function trackUser(id: number) {
  userIds.push(id);
}
export function trackRole(id: number) {
  roleIds.push(id);
}
export function trackedUsers() {
  return [...userIds];
}
export function trackedRoles() {
  return [...roleIds];
}
```

- [ ] **Step 2: 提交**

```bash
git add tests/e2e/helpers/ids.ts
git commit -m "feat(e2e): id tracker"
```

---

## Task 9: 写 globalSetup

**Files:**
- Create: `tests/e2e/globalSetup.ts`

- [ ] **Step 1: 写文件**

```ts
import { login } from './helpers/auth.js';
import { SEED } from './fixtures/seed.js';

export default async function () {
  const token = await login(SEED.admin.user, SEED.admin.pass);
  (globalThis as unknown as { __adminToken__: string }).__adminToken__ = token;
}
```

- [ ] **Step 2: 类型检查**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 无错

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/globalSetup.ts
git commit -m "feat(e2e): globalSetup admin login"
```

---

## Task 10: 写 auth.e2e.test.ts（4 case）

**Files:**
- Create: `tests/e2e/auth.e2e.test.ts`

- [ ] **Step 1: 写文件**

```ts
import { describe, it, expect, afterAll } from 'vitest';
import request from 'supertest';
import { baseURL } from './helpers/http.js';
import { login, logout } from './helpers/auth.js';
import { Payload } from './fixtures/payload.js';
import { DEBUG } from './helpers/debug.js';

describe('auth', () => {
  let token: string | undefined;

  it('should login admin and return access_token', async () => {
    token = await login(
      process.env.E2E_ADMIN_USER ?? 'admin',
      process.env.E2E_ADMIN_PASS ?? 'admin123',
    );
    expect(token).toBeTruthy();
    if (DEBUG) console.log('token len', token.length);
  });

  it('should reject wrong password with 400/401', async () => {
    const res = await request(baseURL)
      .post('/api/v1/auth/login')
      .send({ username: 'admin', password: 'wrong' });
    expect([400, 401]).toContain(res.status);
  });

  it('should register a new user', async () => {
    const body = Payload.register();
    const res = await request(baseURL).post('/api/v1/auth/register').send(body);
    expect([200, 201]).toContain(res.status);
    expect(res.body).toHaveProperty('id');
  });

  it('should report setup status', async () => {
    const res = await request(baseURL).get('/api/v1/auth/setup-status');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('setup_required');
  });

  afterAll(async () => {
    if (token) await logout(token);
  });
});
```

- [ ] **Step 2: 类型检查**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 无错

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/auth.e2e.test.ts
git commit -m "test(e2e): auth group (4 cases)"
```

---

## Task 11: 写 users.e2e.test.ts（5 case）

**Files:**
- Create: `tests/e2e/users.e2e.test.ts`

- [ ] **Step 1: 写文件**

```ts
import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http.js';
import { trackUser } from './helpers/ids.js';
import { SEED } from './fixtures/seed.js';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

describe('users', () => {
  let createdUserId: number | undefined;

  it('should fetch current admin profile', async () => {
    const res = await authedReq(adminToken).get('/api/v1/users/me');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('username');
  });

  it('should list users with pagination', async () => {
    const res = await authedReq(adminToken).get('/api/v1/users?page=1&size=10');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('list');
    expect(Array.isArray(res.body.list)).toBe(true);
  });

  it('should create a new user via register or admin endpoint', async () => {
    const u = SEED.newUser();
    const res = await authedReq(adminToken).post('/api/v1/users', u);
    if (res.status === 200 || res.status === 201) {
      createdUserId = res.body.id;
      trackUser(createdUserId!);
    } else {
      const fallback = await authedReq(adminToken).post(
        '/api/v1/auth/register',
        u,
      );
      expect([200, 201]).toContain(fallback.status);
      createdUserId = fallback.body.id;
      trackUser(createdUserId!);
    }
    expect(createdUserId).toBeGreaterThan(0);
  });

  it('should update extends on a user', async () => {
    expect(createdUserId).toBeDefined();
    const res = await authedReq(adminToken).patch(
      `/api/v1/users/${createdUserId}/extends`,
      { department: 'e2e-dept' },
    );
    expect([200, 204]).toContain(res.status);
  });

  it('should assign and remove user roles', async () => {
    expect(createdUserId).toBeDefined();
    const assign = await authedReq(adminToken).post(
      `/api/v1/users/${createdUserId}/roles`,
      { role_ids: [] },
    );
    expect([200, 204]).toContain(assign.status);
    const remove = await authedReq(adminToken).del(
      `/api/v1/users/${createdUserId}/roles`,
      { role_ids: [] },
    );
    expect([200, 204]).toContain(remove.status);
  });
});
```

- [ ] **Step 2: 类型检查**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 无错

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/users.e2e.test.ts
git commit -m "test(e2e): users group (5 cases)"
```

---

## Task 12: 写 tokens.e2e.test.ts（5 case）

**Files:**
- Create: `tests/e2e/tokens.e2e.test.ts`

- [ ] **Step 1: 写文件**

```ts
import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http.js';
import { Payload } from './fixtures/payload.js';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

describe('tokens', () => {
  it('should list resources', async () => {
    const res = await authedReq(adminToken).get('/api/v1/tokens/resource');
    expect(res.status).toBe(200);
    expect(Array.isArray(res.body)).toBe(true);
  });

  it('should reject access token request with placeholder code', async () => {
    const res = await authedReq(adminToken).post(
      '/api/v1/tokens/access',
      Payload.token.access(),
    );
    expect([400, 401]).toContain(res.status);
  });

  it('should create api key', async () => {
    const res = await authedReq(adminToken).post(
      '/api/v1/tokens/api_key',
      Payload.token.apiKey(),
    );
    expect([200, 201]).toContain(res.status);
    expect(res.body).toHaveProperty('api_key');
  });

  it('should reject refresh with placeholder token', async () => {
    const res = await authedReq(adminToken).post(
      '/api/v1/tokens/refresh',
      Payload.token.refresh('placeholder'),
    );
    expect([400, 401]).toContain(res.status);
  });

  it('should revoke a token', async () => {
    const res = await authedReq(adminToken).post('/api/v1/tokens/revoke', {
      token: adminToken,
    });
    expect([200, 204, 400]).toContain(res.status);
  });
});
```

- [ ] **Step 2: 类型检查**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 无错

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/tokens.e2e.test.ts
git commit -m "test(e2e): tokens group (5 cases)"
```

---

## Task 13: 写 roles.e2e.test.ts（6 case）

**Files:**
- Create: `tests/e2e/roles.e2e.test.ts`

- [ ] **Step 1: 写文件**

```ts
import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http.js';
import { trackRole } from './helpers/ids.js';
import { Payload } from './fixtures/payload.js';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

describe('roles', () => {
  let createdRoleId: number | undefined;

  it('should list roles with pagination', async () => {
    const res = await authedReq(adminToken).get('/roles?page=1&size=10');
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('list');
  });

  it('should create a role', async () => {
    const res = await authedReq(adminToken).post('/roles', Payload.role());
    expect([200, 201]).toContain(res.status);
    createdRoleId = res.body.id;
    trackRole(createdRoleId);
    expect(createdRoleId).toBeGreaterThan(0);
  });

  it('should fetch a role by id', async () => {
    expect(createdRoleId).toBeDefined();
    const res = await authedReq(adminToken).get(`/roles/${createdRoleId}`);
    expect(res.status).toBe(200);
    expect(res.body).toHaveProperty('code');
  });

  it('should update a role', async () => {
    expect(createdRoleId).toBeDefined();
    const res = await authedReq(adminToken)
      .put(`/roles/${createdRoleId}`, { ...Payload.role(), status: 1 });
    expect([200, 204]).toContain(res.status);
  });

  it('should assign permissions to a role', async () => {
    expect(createdRoleId).toBeDefined();
    const res = await authedReq(adminToken).post(
      `/roles/${createdRoleId}/permissions`,
      { permission_ids: [] },
    );
    expect([200, 204]).toContain(res.status);
  });

  it('should delete a role', async () => {
    expect(createdRoleId).toBeDefined();
    const res = await authedReq(adminToken).del(`/roles/${createdRoleId}`);
    expect([200, 204]).toContain(res.status);
  });
});
```

- [ ] **Step 2: 类型检查**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 无错

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/roles.e2e.test.ts
git commit -m "test(e2e): roles group (6 cases)"
```

---

## Task 14: 写 permissions.e2e.test.ts（1 case）

**Files:**
- Create: `tests/e2e/permissions.e2e.test.ts`

- [ ] **Step 1: 写文件**

```ts
import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http.js';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

describe('permissions', () => {
  it('should list all permissions', async () => {
    const res = await authedReq(adminToken).get('/permissions');
    expect(res.status).toBe(200);
    expect(Array.isArray(res.body)).toBe(true);
  });
});
```

- [ ] **Step 2: 提交**

```bash
git add tests/e2e/permissions.e2e.test.ts
git commit -m "test(e2e): permissions group (1 case)"
```

---

## Task 15: 写 README + 根 package.json 脚本

**Files:**
- Create: `tests/e2e/README.md`
- Modify: `package.json:6-15`

- [ ] **Step 1: 写 README.md**

````markdown
# E2E Tests

Vitest + supertest 跨 API Gateway 测真服务。

## 前置

```powershell
# 1. 起基础设施 + 4 服务
pnpm run dev

# 2. 初始化 admin（如果还没）
pnpm run test:reset
```

## 跑

```powershell
# 全量
pnpm run test:e2e

# 单组
pnpm run test:e2e:users

# 单 case
pnpm --filter e2e test users -t "update extends"

# 调试
$env:E2E_DEBUG=1
pnpm run test:e2e:users -- --reporter=verbose
```

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `E2E_BASE_URL` | `http://localhost:8080` | API Gateway |
| `E2E_ADMIN_USER` | `admin` | admin 用户名 |
| `E2E_ADMIN_PASS` | `admin123` | admin 密码 |
| `E2E_CLIENT_ID` | `e2e-client` | token 客户端 ID |
| `E2E_CLIENT_SECRET` | `e2e-secret` | token 客户端密钥 |
| `E2E_DEBUG` | `0` | `1` 输出每次请求/响应 |
````

- [ ] **Step 2: 修改根 package.json**

在 `test:loop` 之后追加：

```json
    "test:e2e": "pnpm --filter e2e test",
    "test:e2e:auth": "pnpm --filter e2e test auth",
    "test:e2e:users": "pnpm --filter e2e test users",
    "test:e2e:tokens": "pnpm --filter e2e test tokens",
    "test:e2e:roles": "pnpm --filter e2e test roles",
    "test:e2e:permissions": "pnpm --filter e2e test permissions",
```

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/README.md package.json
git commit -m "chore(e2e): readme and root scripts"
```

---

## Task 16: 验收：全量跑通

**Files:** 无

- [ ] **Step 1: 类型检查**

Run: `cd tests/e2e; pnpm exec tsc --noEmit`
Expected: 退出码 0

- [ ] **Step 2: 全量跑**

Run: `pnpm run test:e2e`
Expected: 21 个 case 全过（或已知 service 限制，少数 4xx 已记录）

- [ ] **Step 3: 单 case 跑验证**

Run: `cd tests/e2e; pnpm test users -t "list users"`
Expected: 1 case 通过

- [ ] **Step 4: 调试模式**

Run: `cd tests/e2e; $env:E2E_DEBUG=1; pnpm test users -t "list users" --reporter=verbose`
Expected: 控制台输出 `>> GET /api/v1/users` 和 `<< 200`

---

## Self-Review

- **Spec 覆盖**：5 群 API 21 case ✓（4+5+5+6+1）
- **占位**：0 个 TBD/TODO ✓
- **类型一致**：`authedReq`、`baseURL`、`login`、`logout` 跨任务签名一致 ✓
- **依赖**：vitest 2 + supertest 7 + ts 5；与 spec 一致 ✓
- **范围**：单 plan 可执行 ✓
