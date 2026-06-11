# Node.js Web API E2E 测试设计

**日期**：2026-06-11
**状态**：待审
**作者**：brainstorming

## 背景

`coder-micro-services` 现有 Rust 单元/集成测试（`services/*/tests/`、YAML specs → 测试代码）覆盖单服务逻辑。前端（`frontend/`，Vue3 + TS）已完成调用层封装（`frontend/src/api/{auth,user,token,role,permission}.ts`），但前端集成前缺一个**跨网关**的端到端 HTTP 测试套件，用于：

1. 验证前端 API 调用的真实响应体、状态码与 TypeScript 类型契约一致
2. 跑通多服务协作流程（login → 拿 token → 调业务 → revoke）
3. 给前端开发者一个可重复运行的回归基准

**不在范围内**：浏览器端 UI 测试、单服务内部单元测试、性能/压测、SDK 端到端测试（已有 `sdk/dotnet/tests/`、`sdk/java/`）。

## 目标

- 一次登录 token 复用，全部 5 类 API 共 21 个 case 在串行下通过
- 支持**单文件**、**单 case**、**调试模式**三种粒度独立运行
- 不污染生产数据；不依赖前端打包产物
- 失败时输出请求/响应上下文，定位时间 < 30 秒

## 范围

| API 群 | 端点 | 数量 | 文件 |
|---|---|---|---|
| `auth` | `/api/v1/auth/{login,logout,register,setup-status}` | 4 | `auth.e2e.test.ts` |
| `users` | `/api/v1/users/{me,}`、`/api/v1/users/{id}`、`/api/v1/users/{id}/extends`、`/api/v1/users/{id}/roles` | 5 | `users.e2e.test.ts` |
| `tokens` | `/api/v1/tokens/{access,refresh,revoke,resource,api_key}` | 5 | `tokens.e2e.test.ts` |
| `roles` | `/roles`、`/roles/{id}`、`/roles/{id}/permissions` | 6 | `roles.e2e.test.ts` |
| `permissions` | `/permissions` | 1 | `permissions.e2e.test.ts` |

## 非目标

- 不测试 WS（`notification-service` 含 WS，不在 web-api 范围）
- 不覆盖 `notification-service`、`credential-login-service`（stub 状态）
- 不写契约测试；只发真请求

## 架构

### 目录

```
tests/e2e/
  globalSetup.ts                # 启动后 1 次 admin 登录
  helpers/
    http.ts                     # supertest 封装 + token 注入 + debug 输出
    auth.ts                     # login() / logout() 复用函数
    ids.ts                      # 跨文件共享：记录已创建 user_id/role_id
    seed.ts                     # 预置 admin（读环境变量或 migrations）
  fixtures/
    payload.ts                  # 各 API 请求体常量
  auth.e2e.test.ts
  users.e2e.test.ts
  tokens.e2e.test.ts
  roles.e2e.test.ts
  permissions.e2e.test.ts
  vitest.config.ts              # setupFiles: [globalSetup]
  package.json                  # 子包：vitest + supertest
  tsconfig.json
  README.md                     # 跑测说明
```

### 依赖

- `vitest@^2`（运行 + 报告）
- `supertest@^7`（HTTP 客户端）
- `typescript@^5`（编译）
- 不引入 `axios` / `lodash` / `dotenv`（`process.env` 直读）

### 父包接入

`package.json`（根）新增脚本：
```json
{
  "scripts": {
    "test:e2e": "pnpm --filter e2e test",
    "test:e2e:users": "pnpm --filter e2e test users"
  }
}
```

`pnpm-workspace.yaml` 已含 `tests/*`（待确认；如未含需追加 `tests/e2e`）。

### 依赖服务

**前置**：用户在真实 PowerShell 手动起服务。

文档化 3 种起法：
1. `pnpm run dev:infra` 起 MySQL/Redis/Consul + `pnpm run dev` 起 4 服务
2. `docker compose -f infrastructure/docker-compose.yml up -d`
3. 仅 API Gateway + 后端（手动）

`E2E_BASE_URL` 默认 `http://localhost:8080`（API Gateway）。

## 关键设计

### 1. 共享登录（globalSetup）

```ts
// globalSetup.ts
export default async function () {
  const token = await login(process.env.E2E_ADMIN_USER!, process.env.E2E_ADMIN_PASS!);
  (globalThis as any).__adminToken__ = token;
}
```

**约束**：admin 用户必须存在；不存在时 `globalSetup` 抛错，提示运行 `pnpm run test:reset` 初始化。

### 2. HTTP helper

```ts
// helpers/http.ts
const baseURL = process.env.E2E_BASE_URL ?? 'http://localhost:8080';

export function authedReq(token: string) {
  const agent = request(baseURL);
  return {
    get:  (path: string)        => agent.get(path).set('Authorization', `Bearer ${token}`),
    post: (path: string, data?) => agent.post(path).set('Authorization', `Bearer ${token}`).send(data),
    put:  (path: string, data?) => agent.put(path).set('Authorization', `Bearer ${token}`).send(data),
    patch:(path: string, data?) => agent.patch(path).set('Authorization', `Bearer ${token}`).send(data),
    del:  (path: string, data?) => agent.del(path).set('Authorization', `Bearer ${token}`).send(data),
  };
}
```

### 3. 调试模式

```ts
// 在每个请求前后包一层
if (process.env.E2E_DEBUG) {
  console.log(`>> ${method} ${path}`, data);
  const res = await ...;
  console.log(`<< ${res.status}`, res.body);
  return res;
}
```

只在 `E2E_DEBUG=1` 时生效，默认 0 性能开销。

### 4. 顺序约束

- `auth` 文件独立可跑（无需前置）
- `users` / `tokens` / `roles` / `permissions` 依赖 `globalSetup` 的 admin token
- 文件间**不强制顺序**（vitest 默认并发文件）；不依赖创建顺序的资源

### 5. 失败时停

`vitest --bail=1` 由用户传 flag；脚本里**不**自动 bail（避免半套数据）。

## 跑测

### 全量

```powershell
pnpm run test:e2e
```

### 单组

```powershell
pnpm run test:e2e:users
# 等价
pnpm --filter e2e test users
```

### 单 case

```powershell
pnpm --filter e2e test users -t "should update me"
```

### 调试

```powershell
$env:E2E_DEBUG=1
$env:E2E_BASE_URL="http://localhost:8080"
pnpm --filter e2e test users --reporter=verbose
```

## 错误处理

| 场景 | 行为 |
|---|---|
| 服务未起 | `globalSetup` 抛 `ECONNREFUSED`，提示起服务 |
| admin 不存在 | `globalSetup` 抛 `401`，提示跑 `test:reset` |
| case 失败 | 输出请求/响应体；保留创建过的资源不清理（依赖 `E2E_PAUSE`） |
| 编译错 | `tsc --noEmit` 在 `pnpm test` 前跑 |

## 不做的事（YAGNI）

- 不做并发文件执行优化（默认即可）
- 不做覆盖率报告（避免引入 v8 istanbul）
- 不做 mock server（前提是服务必须真跑）
- 不做测试数据回滚（依赖 DB 隔离 + fixtures 幂等）
- 不做 CI 集成（`coder-fix-tests` 等 skill 不涉及此目录）

## 验收

- [ ] `pnpm run test:e2e` 21 个 case 全过
- [ ] `pnpm run test:e2e:users` 仅 users 通过
- [ ] `pnpm --filter e2e test users -t "update me"` 单 case 通过
- [ ] `E2E_DEBUG=1` 失败时输出请求/响应体
- [ ] 失败用例定位时间 < 30s
- [ ] 不污染 `migrations` 已存在数据
