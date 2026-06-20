# E2E 测试使用文档

跨 API Gateway 对真实微服务做端到端测试。包含两套体系：

1. **传统断言用例**（`*.e2e.test.ts`）：Vitest + supertest，按服务分文件，代码内手写断言。
2. **数据驱动用例**（`api/` + `ui/`）：用例写在 Excel，执行器遍历执行，产出 Excel 报告。
   - `api/`：直连 api-gateway，纯接口验证。
   - `ui/`：通过前端页面走业务流（Playwright）。

两套共享底层设施 `shared/`（环境配置、数据库重置、Excel 读写、报告）。

---

## 1. 前置条件

### 1.1 启动基础设施（强制 infrastructure 目录）

所有 e2e 必须基于 `infrastructure/` 目录运行，容器名以 `infrastructure-` 前缀。**禁止**用旧的 `docker/` 路径。

```powershell
cd D:\projects\coder-micro-services\infrastructure
docker compose up -d

# 网络抖动时跳过拉镜像
docker compose up -d --pull never
```

服务端口：

| 服务 | 外部端口 | 内部端口 |
|---|---|---|
| api-gateway | 3000 | 3000 |
| token-service | 6001 | 8080 |
| iam-service | 6002 | 8080 |
| notification-service | 6003 | 8080 |
| audit/prue_backend | 6004 | 8080 |
| consul | 8500 | — |
| mysql | 3306 | — |
| redis | 6379 | — |

### 1.2 安装依赖

`tests/e2e` 是独立 workspace（绕过 frontend 的 engines/pnpm 限制）。

```powershell
pnpm -C tests/e2e install
```

### 1.3 UI 测试额外要求

- 前端容器需就绪（默认 `http://localhost:8080`），`ui/globalSetup.ts` 会做健康探测，失败即中止。
- 前端必须禁用登录验证码：`infrastructure/docker-compose.yml` 已为 frontend 构建注入 `VITE_DISABLE_CAPTCHA=true`，改动后需 `docker compose build frontend`。
- Playwright 使用完整 chromium（`channel: 'chromium'`），首次需 `pnpm -C tests/e2e exec playwright install chromium`。

---

## 2. 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `E2E_BASE_URL` | `http://localhost:3000` | api-gateway 入口 |
| `E2E_FRONTEND_URL` | `http://localhost:8080` | 前端入口（UI 测试用） |
| `E2E_ADMIN_USER` | `e2eadmin` | globalSetup 自注册的 admin 用户名 |
| `E2E_ADMIN_PASS` | `P@ssw0rd!` | admin 密码 |
| `E2E_DEBUG` | `0` | `1` 输出每次请求/响应日志 |
| `E2E_DB_HOST` | `localhost` | MySQL 主机 |
| `E2E_DB_PORT` | `3306` | MySQL 端口 |
| `E2E_DB_USER` | `root` | MySQL 用户 |
| `E2E_DB_PASSWORD` | `rootpass` | MySQL 密码 |
| `E2E_DB_NAME` | `superpowers` | 数据库名 |
| `E2E_SEED_USERNAMES` | `e2eadmin` | `resetBusiness()` 保留的基础用户名（逗号分隔） |

> 集中读取见 [config.ts](file:///d:/projects/coder-micro-services/tests/e2e/shared/config.ts)。

---

## 3. 运行命令

所有命令在 `tests/e2e` 内执行，或加 `-C tests/e2e`。

### 3.1 传统断言用例（Vitest）

```powershell
# 全量
pnpm -C tests/e2e test

# watch 模式
pnpm -C tests/e2e test:watch

# 分组（按服务）
pnpm -C tests/e2e test:auth
pnpm -C tests/e2e test:users
pnpm -C tests/e2e test:roles
pnpm -C tests/e2e test:permissions
pnpm -C tests/e2e test:tokens
pnpm -C tests/e2e test:audit
pnpm -C tests/e2e test:config
pnpm -C tests/e2e test:file
pnpm -C tests/e2e test:notification
pnpm -C tests/e2e test:token-admin
pnpm -C tests/e2e test:menu-configs
pnpm -C tests/e2e test:iam-orgs
pnpm -C tests/e2e test:gateway

# 单 case（-t 过滤用例名）
pnpm -C tests/e2e test users -t "update extends"
```

### 3.2 数据驱动用例

```powershell
# API 数据驱动（遍历 api/cases/*.xlsx）
pnpm -C tests/e2e test:api

# UI 业务流（遍历 ui/cases/*.xlsx，需前端容器）
pnpm -C tests/e2e test:ui

# 重新生成样例用例 xlsx
pnpm -C tests/e2e gen:samples
```

### 3.3 调试模式

```powershell
$env:E2E_DEBUG=1
pnpm -C tests/e2e test users -t "list users" --reporter=verbose
```

输出形如：

```
>> GET /api/v1/users?page=1&size=10
<< 200 { list: [...], total: 5, page: 1, size: 10 }
```

---

## 4. 目录结构

```
tests/e2e/
  globalSetup.ts             # 传统用例：清用户表 + 注册并登录 admin，token 写入 .e2e-admin.json
  setupFile.ts               # 每个测试文件加载，读 .e2e-admin.json 注入 globalThis.__adminToken__
  vitest.config.ts           # 传统用例配置（globalSetup + setupFiles + reporter）

  helpers/                   # 传统用例公共方法
    http.ts                  # authedReq(token).get/post/put/patch/del（supertest 封装）
    auth.ts                  # login() / logout()
    db.ts                    # clearUsers() / queryUserByUsername() / queryUserById()
    ids.ts                   # 跨 case 共享 id 记录（trackUser/trackedUsers 等）
    debug.ts                 # E2E_DEBUG 日志
  fixtures/
    seed.ts                  # admin 账号 + 随机新用户/角色
    payload.ts               # 测试请求体常量

  *.e2e.test.ts              # 按服务分文件（auth/users/tokens/roles/permissions/audit/
                             #   config/file/notification/token-admin/menu-configs/
                             #   iam-org-units/gateway/user-flow）

  shared/                    # 数据驱动共享设施
    config.ts                # 集中环境变量
    db/
      reset.ts               # resetBase()（清全部+重导 seed）/ resetBusiness()（清业务保留 admin）
      seed/base.sql          # 基础数据（admin 由 API 注册，此处可追加角色/菜单等）
    data/excel.ts            # readCases() 读 xlsx + resolvePlaceholders() + assertBody()
    report/
      writer.ts              # writeReport() 写 report-<时间戳>.xlsx；writeErrorJson() 写失败明细
      errors/                # 失败明细（<caseId>.json，UI 附 <caseId>.png）

  api/                       # 数据驱动 - API
    cases/*.xlsx             # 用例（每 sheet 一组，每行一用例）
    globalSetup.ts           # resetBase + 注册并登录 admin，token 写 .token.json
    runner/run.e2e.test.ts   # 遍历 xlsx 执行 + 断言 + 收集结果
    vitest.config.ts

  ui/                        # 数据驱动 - UI
    cases/*.xlsx             # 用例（flow + params + expect）
    pages/                   # Page Object（login.page.ts / user.page.ts）
    flows/index.ts           # 业务流编排注册表（flow 名 → 函数）
    globalSetup.ts           # resetBase + 注册 admin + 前端健康探测
    runner/run.spec.ts       # 遍历 xlsx 执行 flow + 校验 toast
    playwright.config.ts

  scripts/gen-sample-cases.mjs  # 生成样例 xlsx
```

---

## 5. 数据分层与重置

| 函数 | 时机 | 行为 |
|---|---|---|
| `resetBase()` | 整轮测试前（globalSetup） | 关外键检查 → 清 `BUSINESS_TABLES` + `users` → 重导 `seed/base.sql` |
| `resetBusiness()` | 每套件前（`beforeAll`） | 清 `BUSINESS_TABLES` → 删除非 `seedUsernames` 的用户，保留 admin |

- admin 账号始终由 API `POST /v1/iam/auth/register` 创建（首个用户即 admin），保证密码哈希与服务一致，不直接 INSERT。
- 业务表清单见 [reset.ts](file:///d:/projects/coder-micro-services/tests/e2e/shared/db/reset.ts#L9-L22)。

---

## 6. 编写传统断言用例

新建 `<service>.e2e.test.ts`，用 `authedReq` 发请求，Vitest 断言。

```ts
import { describe, it, expect } from 'vitest';
import { authedReq } from './helpers/http';

const token = (globalThis as any).__adminToken__ as string;

describe('iam users', () => {
  it('list users', async () => {
    const res = await authedReq(token).get('/v1/iam/users?page=1&size=10');
    expect(res.status).toBe(200);
    expect(res.body.list).toBeInstanceOf(Array);
  });
});
```

- token 由 `setupFile.ts` 注入到 `globalThis.__adminToken__`。
- 跨用例传递 id 用 `helpers/ids.ts` 的 `trackUser()` / `trackedUsers()` 等。
- 路径以 `/v1/<service>/<resource>` 开头（网关已剥除 `/api`）。

---

## 7. 编写数据驱动用例

### 7.1 API 用例（Excel）

文件 `api/cases/*.xlsx`，每 sheet 一组，列定义见 [excel.ts `ApiCase`](file:///d:/projects/coder-micro-services/tests/e2e/shared/data/excel.ts#L4-L14)：

| 列 | 必填 | 说明 |
|---|---|---|
| `caseId` | 是 | 唯一 ID，用于占位引用与失败明细命名 |
| `desc` | 是 | 用例描述 |
| `method` | 是 | GET/POST/PUT/PATCH/DELETE |
| `path` | 是 | 请求路径，支持占位符 |
| `headers` | 否 | （预留） |
| `body` | 否 | JSON 字符串，支持占位符 |
| `expectStatus` | 是 | 期望 HTTP 状态码 |
| `expectBody` | 否 | 断言表达式 |
| `skip` | 否 | `Y` 跳过该用例 |

### 7.2 占位符

- `${rand}`：每次替换为唯一值（时间戳+随机）。
- `${CASE-ID.field}`：引用前序用例响应体字段，支持 `a.b.c` 路径。例 `${IAM-USER-001.id}`。

### 7.3 断言表达式（expectBody）

- `$.field exists`：字段存在且非 null。
- `$.field==value`：字段字符串相等。
- 多条用 `&&` 连接，例：`$.id exists && $.username==alice`。

### 7.4 UI 用例（Excel）

文件 `ui/cases/*.xlsx`，列：`caseId` / `desc` / `flow` / `params` / `expect` / `skip`。

- `flow`：必须在 [ui/flows/index.ts](file:///d:/projects/coder-micro-services/tests/e2e/ui/flows/index.ts) 注册。
- `params`：JSON 字符串，支持占位符。
- `expect`：当前支持 `toast=<子串>`，校验页面提示包含该子串。

新增 flow：在 `flows` 注册表加函数，复用 `pages/` 下 Page Object。

### 7.5 样例

参考 [gen-sample-cases.mjs](file:///d:/projects/coder-micro-services/tests/e2e/scripts/gen-sample-cases.mjs) 生成的：

- `api/cases/iam-users.xlsx`：创建→查详情→删除。
- `ui/cases/user-flow.xlsx`：登录后建用户，校验 `toast=已新增用户`。

修改后直接重跑 `test:api` / `test:ui`，无需改执行器代码。

---

## 8. 测试报告

数据驱动测试结束后（`afterAll`）自动写报告：

- `shared/report/report-<时间戳>.xlsx`：含 `detail`（每用例结果）和 `summary`（total/pass/fail/skip）两个 sheet。
- 失败明细：`shared/report/errors/<caseId>.json`（含 request/response/expect/timestamp）。
- UI 失败附截图：`shared/report/errors/<caseId>.png`。

控制台结束时打印报告路径。

---

## 9. AI / 协作边界

- 改 `tests/e2e/**` 即可修测试。
- **禁止**改 `services/<svc>/src/`、`libs/`、`sdk/` 业务源码。
- 改 `infrastructure/config/api-gateway/routes/` 后必须校验：

```powershell
cargo run -p api-gateway -- validate --routes infrastructure\config\api-gateway\routes
```
