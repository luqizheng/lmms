# E2E 测试使用文档

跨 API Gateway 对真实微服务做端到端测试。全部用例数据驱动，写在 YAML，执行器遍历执行，产出 Excel 报告：

- `api/`：直连 api-gateway，纯接口验证（Vitest + supertest）。

共享底层设施 `shared/`（环境配置、数据库重置、YAML 读取、报告）。

> 历史说明：框架不再随附前端控制台（PRISM console 已移除），原 Playwright UI 测试套件已随
> 之下线。业务方前端在自己的仓库里开发，用 `@digua/contracts` + `@digua/http`（`packages/`）
> 或 `sdk/`（Java / Rust）访问本框架。

---

## 1. 前置条件

### 1.1 启动基础设施（强制 infrastructure 目录）

所有 e2e 必须基于 `infrastructure/` 目录运行，容器名以 `infrastructure-` 前缀。**禁止**用旧的 `docker-example/` 路径。

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

```powershell
pnpm -C tests/e2e install
```

---

## 2. 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `E2E_BASE_URL` | `http://localhost:3000` | api-gateway 入口 |
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

### 3.1 数据驱动用例

```powershell
# 全量
pnpm -C tests/e2e test

# API 数据驱动（遍历 api/cases/*.yaml）
pnpm -C tests/e2e test:api
```

### 3.2 调试模式

```powershell
$env:E2E_DEBUG=1
pnpm -C tests/e2e test:api
```

---

## 4. 目录结构

```
tests/e2e/
  shared/                    # 共享设施
    config.ts                # 集中环境变量
    db/
      reset.ts               # resetBase()（清全部+重导 seed）/ resetBusiness()（清业务保留 admin）
      seed/base.sql          # 基础数据（admin 由 API 注册，此处可追加角色/菜单等）
    data/cases.ts            # readCases() 读 yaml + resolve() 深层占位符 + assertBody()
    report/
      writer.ts              # writeReport() 写 report-<时间戳>.xlsx；writeErrorJson() 写失败明细
      errors/                # 失败明细（<caseId>.json）

  api/                       # 数据驱动 - API
    cases/*.yaml             # 用例（group + cases 数组）
    globalSetup.ts           # resetBase + 注册并登录 admin，token 写 .token.json
    runner/run.e2e.test.ts   # 遍历 yaml 执行 + 断言 + 收集结果
    vitest.config.ts
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

## 6. 编写数据驱动用例

### 6.1 API 用例（YAML）

文件 `api/cases/*.yaml`，顶层 `group` + `cases` 数组，字段见 [cases.ts `ApiCase`](file:///d:/projects/coder-micro-services/tests/e2e/shared/data/cases.ts#L5-L15)：

```yaml
group: iam-users
cases:
  - caseId: IAM-USER-001    # 唯一 ID，用于占位引用与失败明细命名
    desc: 创建用户           # 用例描述
    method: POST            # GET/POST/PUT/PATCH/DELETE
    path: /v1/iam/users     # 请求路径，支持占位符
    auth: true              # 可选，false 则不带 Authorization
    headers:                # 可选，附加请求头
      X-Foo: bar
    body:                   # 可选，结构化对象，支持占位符
      username: u${rand}
    expectStatus: 200       # 期望状态码，可为单值或数组 [200,502,404]
    expect:                 # 可选，断言数组
      - $.id exists
    skip: false             # 可选，true 跳过
```

### 6.2 占位符

- `${rand}`：每次替换为唯一值（时间戳+随机）。
- `${CASE-ID.field}`：引用前序用例响应体字段，支持 `a.b.c` 路径。例 `${IAM-USER-001.id}`。
- 占位符递归作用于 `path` / `body` / `params` / `expect` 的任意层级。

### 6.3 断言（expect）

字符串数组，每条独立判定：

- `$.field exists` / `$.field not exists`：字段存在性。
- `$.field==value`：字段字符串相等。
- `$.field is array`：字段为数组（`$` 指根）。
- `$.field contains 子串`：序列化后包含子串（`$` 指根）。

### 6.4 样例

- [api/cases/iam-users.yaml](file:///d:/projects/coder-micro-services/tests/e2e/api/cases/iam-users.yaml)：创建→查详情→删除。

修改后直接重跑 `test:api`，无需改执行器代码。

---

## 7. 测试报告

数据驱动测试结束后（`afterAll`）自动写报告：

- `shared/report/report-<时间戳>.xlsx`：含 `detail`（每用例结果）和 `summary`（total/pass/fail/skip）两个 sheet。
- 失败明细：`shared/report/errors/<caseId>.json`（含 request/response/expect/timestamp）。

控制台结束时打印报告路径。

---

## 8. AI / 协作边界

- 改 `tests/e2e/**` 即可修测试。
- **禁止**改 `services/<svc>/src/`、`libs/`、`sdk/` 业务源码。
- 改 `infrastructure/config/api-gateway/routes/` 后必须校验：

```powershell
cargo run -p api-gateway -- validate --routes infrastructure\config\api-gateway\routes
```
