---
name: "coder-e2e-test"
description: "Runs E2E test suite for login and authentication flows. Invoke when user asks to run E2E tests, verify login flow, or check end-to-end scenarios."
---

# coder-e2e-test

## 何时用

用户说「跑E2E测试」「验证登录流程」「端到端测试」时调用本skill。

## 前置

- 运行环境：WSL / macOS / Linux（Windows 必须在 WSL 内执行）
- Docker 已启动
- Node.js 18+ / pnpm
- 所有命令均在**项目根目录**执行，禁止使用绝对路径

## 目录检查（每次执行前必做）

```bash
# 确认当前在项目根目录（应能看到 infrastructure / scripts / tests 目录）
pwd
test -d infrastructure && test -f scripts/run-e2e-local.js
```

若不在根目录，先切回：`cd <仓库根>`。

## 步骤

### 1. 启动基础设施

```bash
docker compose -f infrastructure/docker-compose.yml up -d
```

服务端口：api-gateway 3000 / token 6001 / iam 6002 / notification 6003 / prue_backend 6004 / consul 8500 / mysql 3306 / redis 6379。

### 2. 安装 e2e 依赖

```bash
pnpm -C tests/e2e install
```

### 3. 运行 E2E 测试

```bash
node scripts/run-e2e-local.js
```

`globalSetup.ts` 会自动注册并登录 e2e admin（`e2eadmin` / `P@ssw0rd!`）。

## 常用场景

`run-e2e-local.js` 后接参数透传给 vitest（按文件名过滤）：

```bash
# 验证认证流程
node scripts/run-e2e-local.js auth

# 验证用户
node scripts/run-e2e-local.js users

# 单个用例 + 调试日志
E2E_DEBUG=1 node scripts/run-e2e-local.js users -t "list users"
```

可用过滤词：auth / users / roles / permissions / tokens / audit / config / file / notification / token-admin / menu-configs / iam-org-units / gateway。

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `E2E_BASE_URL` | `http://localhost:3000` | api-gateway 入口 |
| `E2E_ADMIN_USER` | `e2eadmin` | globalSetup 自注册 |
| `E2E_ADMIN_PASS` | `P@ssw0rd!` | globalSetup 自注册 |
| `E2E_DEBUG` | `0` | 1=请求/响应日志 |

## 故障排查

- `MODULE_NOT_FOUND`：确认用 `scripts/run-e2e-local.js`，不存在 `test-orchestrator.js` / `run-e2e.js`
- 测试失败：`E2E_DEBUG=1` 重跑看请求日志
- 基础设施问题：`docker ps` 检查 `infrastructure-` 前缀容器状态
