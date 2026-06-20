# E2E Tests

Vitest + supertest 跨 API Gateway 测真服务。

## 前置

```powershell
# 1. 起基础设施 + 4 服务
pnpm run dev

# 2. 初始化 admin（如果还没）
pnpm run test:reset

# 3. 装 e2e 依赖（独立 workspace，绕过 frontend engines.pnpm）
pnpm -C tests/e2e install
```

> 脚本 `scripts/run-e2e.js` 会自动检测并装 e2e 依赖，这步可省。

## 跑

```powershell
# 全量（21 case）
pnpm run test:e2e

# 单组
pnpm run test:e2e:users

# 单 case
pnpm -C tests/e2e test users -t "update extends"

# 调试
$env:E2E_DEBUG=1
pnpm -C tests/e2e test users --reporter=verbose
```

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `E2E_BASE_URL` | `http://localhost:3000` | API Gateway |
| `E2E_ADMIN_USER` | `admin` | admin 用户名 |
| `E2E_ADMIN_PASS` | `admin123` | admin 密码 |
| `E2E_CLIENT_ID` | `e2e-client` | token 客户端 ID |
| `E2E_CLIENT_SECRET` | `e2e-secret` | token 客户端密钥 |
| `E2E_DEBUG` | `0` | `1` 输出每次请求/响应 |

## 文件结构

```
tests/e2e/
  globalSetup.ts             # admin 登录一次
  helpers/
    http.ts                  # authedReq(token).get/post/put/patch/del
    auth.ts                  # login() / logout()
    ids.ts                   # 跨 case 共享 id 记录
    debug.ts                 # E2E_DEBUG 日志
  fixtures/
    seed.ts                  # admin 账号 + 随机新用户/角色
    payload.ts               # 测试请求体常量
  auth.e2e.test.ts           # iam-service 认证
  users.e2e.test.ts          # iam-service 用户
  tokens.e2e.test.ts         # token-service 基础 token
  roles.e2e.test.ts          # iam-service 角色
  permissions.e2e.test.ts    # iam-service 权限
  audit.e2e.test.ts          # audit-service 操作/登录/保留策略
  config.e2e.test.ts         # config-service namespace + item CRUD
  file.e2e.test.ts           # file-service 上传/列表/预签名
  notification.e2e.test.ts   # notification-service 公告全生命周期
  token-admin.e2e.test.ts    # token-service oauth-client + JWKS + user-jwt
  iam-menus.e2e.test.ts      # iam-service 菜单 CRUD
  iam-org-units.e2e.test.ts  # iam-service 组织 CRUD
  gateway.e2e.test.ts        # api-gateway 健康路由 + 401 拦截
```

## 分组运行

```powershell
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
pnpm -C tests/e2e test:iam-menus
pnpm -C tests/e2e test:iam-orgs
pnpm -C tests/e2e test:gateway
```

## 调试模式示例

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

## 数据驱动重构（两部分）

测试拆为两部分，共享底层设施（`shared/`）。

### 目录

```
tests/e2e/
  shared/
    config.ts               # 集中环境变量（baseURL/frontendURL/admin/db）
    db/
      reset.ts              # resetBase()/resetBusiness()
      seed/base.sql         # 基础数据（admin 由 API 注册）
    data/excel.ts           # 读用例 xlsx + 占位符 + 断言
    report/
      writer.ts             # 写 report-<时间戳>.xlsx + 失败 <caseId>.json
      errors/               # 失败明细（json + UI 截图）
  api/                      # 第一部分：直连 api-gateway
    cases/*.xlsx            # 数据驱动用例
    globalSetup.ts          # resetBase + 注册并登录 admin
    runner/run.e2e.test.ts  # 遍历 xlsx 执行
    vitest.config.ts
  ui/                       # 第二部分：通过前端做业务测试
    cases/*.xlsx
    pages/                  # Page Object（login/user）
    flows/                  # 业务流编排
    globalSetup.ts          # resetBase + 注册 admin + 前端健康探测
    runner/run.spec.ts
    playwright.config.ts
  scripts/gen-sample-cases.mjs  # 生成样例 xlsx
```

### 数据分层

- 基础数据：`resetBase()` 整轮前清全部 + 注册 admin（幂等）。
- 业务数据：`resetBusiness()` 每套件前清业务表，保留 admin。

### 用例与报告（Excel）

- 用例：`api/cases/*.xlsx`、`ui/cases/*.xlsx`，每 sheet 一组，每行一用例。
- 占位符：`${rand}` 唯一值；`${CASE-ID.field}` 引用前序响应。
- 断言：`$.x exists` / `$.x==v`，多条用 `&&`。
- 报告：`shared/report/report-<时间戳>.xlsx`（detail + summary）。
- 失败：`shared/report/errors/<caseId>.json`（UI 附 `.png`）。

### 运行

```powershell
pnpm -C tests/e2e test:api      # 第一部分：API 数据驱动
pnpm -C tests/e2e test:ui       # 第二部分：前端业务（需前端容器）
pnpm -C tests/e2e gen:samples   # 重新生成样例用例
```

> UI 测试要求前端容器禁用登录验证码：`infrastructure/docker-compose.yml`
> 已为 frontend 构建传入 `VITE_DISABLE_CAPTCHA=true`，改后需 `docker compose build frontend`。

### 新增用例

1. 编辑或新增 `api/cases/*.xlsx` / `ui/cases/*.xlsx`（列见上文）。
2. UI 用例的 `flow` 字段需在 `ui/flows/index.ts` 注册对应编排。
3. 直接重跑 `test:api` / `test:ui`，无需改执行器代码。

