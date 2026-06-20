# E2E 测试重构设计

日期：2026-06-20
状态：已确认设计，待编写实施计划

## 1. 背景与目标

重构 `tests/e2e`，拆为两部分：

- **Part1 API 直测**：直连 `api-gateway`，验证后端 API 正确性（现有 vitest + supertest 资产，已大部分完成）。
- **Part2 前端业务测试**：通过真实前端程序（Playwright 驱动）完成业务流验证。

数据分两层管理：**基础数据** 与 **业务数据**。
测试用例与测试报告均用 Excel（SheetJS / `xlsx`）管理；失败用例额外输出 `<caseId>.json` 便于排查。

## 2. 技术选型（已确认）

| 项 | 选择 |
|---|---|
| 前端驱动 | Playwright |
| Excel 库 | SheetJS（`xlsx`） |
| 数据重置 | SQL seed + 清表 |
| 项目结构 | 单一 pnpm workspace，内部分 `api/` `ui/` |
| 用例驱动 | 纯数据驱动（Excel 行 → 用例） |

## 3. 目录结构

保留 `tests/e2e` 为单一 pnpm workspace，内部拆 `api/` 与 `ui/`，共享底层设施。

```
tests/e2e/
  shared/                      # 两部分共用
    db/
      reset.ts                 # resetBase() / resetBusiness()
      seed/
        base.sql               # 基础数据：admin/角色/权限/菜单/oauth-client
    data/
      excel.ts                 # SheetJS 读用例 + 写报告
    report/
      writer.ts                # 汇总写 xlsx
      errors/                  # 失败用例 <caseId>.json (+ UI 截图)
    config.ts                  # baseURL/账号/路径等环境变量
  api/                         # Part1：直连 api-gateway（现有资产迁入）
    cases/                     # *.xlsx 用例（按服务分文件）
    runner/                    # 数据驱动执行器（基于 vitest）
    *.e2e.test.ts              # 现有测试逐步并入 runner
  ui/                          # Part2：Playwright 前端业务测试
    cases/                     # *.xlsx 业务流程用例
    pages/                     # Page Object（登录/用户/角色…）
    flows/                     # 业务流编排
    runner/                    # 读 xlsx → test 生成
    playwright.config.ts
  package.json                 # 增 @playwright/test、xlsx
```

## 4. 数据重置机制

两层数据，两种生命周期。

### 4.1 基础数据（一次性，整轮前）

`shared/db/seed/base.sql`，幂等（`INSERT ... ON DUPLICATE KEY UPDATE`）：

- admin 账号、固定角色、权限码、菜单、oauth-client
- 主键用固定值（如 admin id=1），便于断言引用

整轮启动时执行一次（vitest `globalSetup` / Playwright `globalSetup`）：

```
reset.ts → resetBase()
  1. SET FOREIGN_KEY_CHECKS=0
  2. TRUNCATE 业务表 + 基础表
  3. source base.sql
  4. SET FOREIGN_KEY_CHECKS=1
```

### 4.2 业务数据（每套件前）

`reset.ts → resetBusiness()` 只清业务表，保留基础数据：

- 业务表：`users`(非 admin)、`user_roles`(非种子)、`refresh_tokens`、`login_logs`、`announcements`、`config_items`、`operation_logs`、`files` 等 → 整表 DELETE
- 基础表：admin、种子角色、权限、菜单、oauth-client → 不动
- 混入业务行的基础表（如 users）：按 `id > 种子上限` 或 `created_by != seed` 条件删

### 4.3 调用时机

| 时机 | 动作 | 触发点 |
|---|---|---|
| 整轮开始 | resetBase() | globalSetup |
| 每套件前 | resetBusiness() | beforeAll / fixture |
| 用例间 | 不重置 | 靠唯一命名隔离 |

复用现有 `helpers/db.ts` 的 mysql2 连接，扩成上述两函数。

## 5. Excel 用例格式

每个服务/业务流一个 `.xlsx`，每 sheet 一组，每行一个用例。

### 5.1 API 用例列（`api/cases/iam-users.xlsx`）

| 列 | 说明 |
|---|---|
| caseId | 唯一用例号，如 IAM-USER-001 |
| desc | 描述 |
| method | GET/POST/PUT/PATCH/DELETE |
| path | 请求路径，支持占位符 |
| headers | JSON，可空 |
| body | JSON，可空，支持占位符 |
| expectStatus | 期望状态码 |
| expectBody | 极简 JSONPath 断言（`$.x exists` / `$.x==v`） |
| skip | Y 跳过 |

### 5.2 UI 用例列（`ui/cases/user-flow.xlsx`）

| 列 | 说明 |
|---|---|
| caseId | UI-USER-001 |
| desc | 描述 |
| flow | 映射 `ui/flows/*.ts` 的业务流名 |
| params | JSON 入参，支持占位符 |
| expect | UI 断言（toast / 表格 / URL） |
| skip | Y 跳过 |

### 5.3 占位符约定

- `${rand}`：生成唯一值
- `${caseId.field}`：引用前序用例响应（存共享上下文）
- 引用未产出的用例 → 该用例 FAIL 并记 json

## 6. 报告输出

### 6.1 报告 Excel（`shared/report/report-<时间戳>.xlsx`）

明细 sheet：

| caseId | desc | result | status | durationMs | error |
|---|---|---|---|---|---|

末尾 summary sheet：total / pass / fail / skip。

### 6.2 失败明细（`shared/report/errors/<caseId>.json`）

```json
{
  "caseId": "IAM-USER-002",
  "request": { "method": "GET", "path": "...", "headers": {}, "body": null },
  "response": { "status": 500, "body": {} },
  "expect": { "status": 200, "body": "$.username==..." },
  "timestamp": "..."
}
```

UI 失败额外附截图 `errors/<caseId>.png`。

## 7. 执行器

### 7.1 Part1 API（基于现有 vitest）

`api/runner/run.ts`：

1. globalSetup 跑 `resetBase()` + 登录拿 token
2. 遍历 `api/cases/*.xlsx`，每 sheet → `describe`，每行 → `it`
3. 解析占位符 → `authedReq(token)[method](path, body)`
4. 断言 status/body → 收集结果到 reporter
5. 自定义 reporter 汇总写 Excel + 失败 json

现有 `*.e2e.test.ts` 保留，逐步把可数据化的迁进 xlsx；复杂断言仍可留代码。

### 7.2 Part2 UI（Playwright）

`ui/playwright.config.ts`：

1. globalSetup 跑 `resetBase()`
2. `ui/runner` 读 `ui/cases/*.xlsx`，每行 → `test`
3. `flow` 字段映射到 `ui/flows/*.ts`（内部用 `pages/` Page Object 操作真实前端）
4. 断言 UI 状态（toast / 表格 / URL）
5. 失败自动截图 + 写 json，统一汇总 Excel

## 8. 运行命令

```powershell
pnpm -C tests/e2e test:api        # 全量 API
pnpm -C tests/e2e test:api:iam    # 单服务
pnpm -C tests/e2e test:ui         # 全量 UI 业务
pnpm -C tests/e2e test:ui:user    # 单业务流
pnpm -C tests/e2e report:open     # 打开最新 xlsx 报告
```

前置：

- 基础设施 `infrastructure/docker compose up -d`
- UI 还需前端 dev server（或容器 nginx）

新增依赖：`xlsx`(SheetJS)、`@playwright/test`。

## 9. 错误处理 / 边界

- DB 连接失败 → 整轮中止并报错（不静默）
- Excel 缺列/格式错 → 加载期校验，定位到文件 + 行
- 占位符引用未产出的用例 → 该用例 FAIL 并记 json
- UI 前端未启动 → globalSetup 探测健康，失败即中止

## 10. 约束

- 仅改 `tests/e2e/**`，禁止改业务源码（`services/*/src`、`libs/`、`sdk/`）
- 所有 e2e 基于 `infrastructure/` 目录运行
- 改路由后须 `cargo run -p api-gateway -- validate --routes infrastructure\config\api-gateway\routes`
