# tests/e2e AI 协作指南

## 强约束：基础设施目录

**所有 e2e 测试必须基于 `d:\projects\coder-micro-services\infrastructure` 目录运行。**

- Docker Compose：`infrastructure/docker-compose.yml`
- 路由配置：`infrastructure/config/api-gateway/routes/`
- nginx：`infrastructure/nginx/`
- 镜像清单：`infrastructure/base-images.json`

**禁止** 使用 `docker-example/`、`docker-compose.yml`（旧）等其它路径启动服务。容器名固定以 `infrastructure-` 前缀。

## 启动与运行

> ⚠️ **基础设施必须用 `pnpm run dev:infra:e2e` 启动**（仓库根运行）——它在主入口之上叠加 `docker-compose-infra.dev.yml`，把 3306 等端口发布到宿主。e2e 的 `globalSetup` 从宿主机直连 `localhost:3306` 重置库，**只跑 `docker compose up -d` 会卡在 ECONNREFUSED**。等价 docker 命令见脚本定义（`package.json` → `scripts.dev:infra:e2e`）。

```powershell
# 1. 启动 infrastructure 基础设施（consul / redis / mysql / api-gateway / token / iam / notification）
pnpm run dev:infra:e2e

# 2. 确认服务端口
# api-gateway  3000
# token        6001
# iam          6002
# notification 6003
# prue_backend 6004
# consul       8500
# mysql        3306
# redis        6379

# 3. 跑 e2e（独立 workspace，绕过 frontend engines.pnpm）
pnpm -C tests/e2e install
pnpm -C tests/e2e test          # 全量：API + UI
pnpm -C tests/e2e test:api      # 仅 API 数据驱动
pnpm -C tests/e2e test:ui       # 仅 UI（需前端容器）
```

## 环境变量（指向 infrastructure）

| 变量 | 默认 | 说明 |
|---|---|---|
| `E2E_BASE_URL` | `http://localhost:3000` | infrastructure-api-gateway |
| `E2E_FRONTEND_URL` | `http://localhost:8080` | 前端容器（UI 测试） |
| `E2E_ADMIN_USER` | `e2eadmin` | globalSetup 自注册 |
| `E2E_ADMIN_PASS` | `P@ssw0rd!` | globalSetup 自注册 |
| `E2E_DEBUG` | `0` | 1=请求/响应日志 |

## 测试结构

全部数据驱动，用例写在 YAML，无手写 `*.e2e.test.ts` 套件。

- `shared/` 共享设施：`config.ts` / `db/reset.ts`（resetBase/resetBusiness）/ `data/cases.ts`（读 yaml+占位符+断言）/ `report/writer.ts`（xlsx 报告）
- `api/`：直连网关。`globalSetup.ts` 注册并登录 admin，`runner/run.e2e.test.ts` 遍历 `cases/*.yaml`
- `ui/`：经前端。`runner/run.spec.ts` 遍历 `cases/*.yaml` 执行 `flows/` 编排；`globalSetup.ts` 额外从 `contracts/permissions.yaml` 重灌 `iam_permissions`（resetBase 会清权限表，角色分配权限等 UI 依赖权限树非空）
- flow 注册按域分文件：`ui/flows/<domain>.ts` 各自 `export const <domain>Flows`，`flows/index.ts` 仅聚合。**新增 flow 只改自己域的文件，不动 index.ts**
- 新增用例 = 编辑 `api/cases/*.yaml` 或 `ui/cases/*.yaml`，无需改执行器

## UI flow 单独验证（开发期，不清库）

```powershell
# 前端用 vite dev（8848）时：
$env:E2E_FRONTEND_URL="http://localhost:8848"
npx tsx ui/dev/run-flow.ts <flowName> '<paramsJson>'   # FLOW_OK / FLOW_FAIL + ui/dev/last-error.png
npx tsx ui/dev/probe.ts <url>                          # 探针：URL/title/body/input 列表
```

`ui/dev/env.ts` 自动探测 `~/playwright-libs`（无 sudo 环境的 chrome 依赖解包目录），其他环境 no-op。

## 调试

```powershell
$env:E2E_DEBUG=1
pnpm -C tests/e2e test:api
```

## AI 操作边界

- 改 `tests/e2e/**` 即可修测试
- **禁止** 改 `services/<svc>/src/`、`libs/`、`sdk/` 业务源码
- 改 `infrastructure/config/api-gateway/routes/` 后必须 `cargo run -p api-gateway -- validate --routes infrastructure\config\api-gateway\routes`