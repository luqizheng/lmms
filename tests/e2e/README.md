# E2E Tests

Vitest + supertest 跨 API Gateway 测真服务。

## 前置

```powershell
# 1. 起基础设施 + 4 服务
pnpm run dev

# 2. 初始化 admin（如果还没）
pnpm run test:reset

# 3. 装 e2e 依赖（独立 workspace，绕过 frontend engines.pnpm）
cd tests/e2e
pnpm install
cd ../..
```

## 跑

```powershell
# 全量（21 case）
pnpm run test:e2e

# 单组
pnpm run test:e2e:users

# 单 case
cd tests/e2e
pnpm test users -t "update extends"

# 调试
$env:E2E_DEBUG=1
cd tests/e2e
pnpm test users --reporter=verbose
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
  auth.e2e.test.ts           # 4 case
  users.e2e.test.ts          # 5 case
  tokens.e2e.test.ts         # 5 case
  roles.e2e.test.ts          # 6 case
  permissions.e2e.test.ts    # 1 case
```

## 调试模式示例

```powershell
$env:E2E_DEBUG=1
cd tests/e2e
pnpm test users -t "list users" --reporter=verbose
```

输出形如：
```
>> GET /api/v1/users?page=1&size=10
<< 200 { list: [...], total: 5, page: 1, size: 10 }
```
