# tests/e2e AI 协作指南

## 强约束：基础设施目录

**所有 e2e 测试必须基于 `d:\projects\coder-micro-services\infrastructure` 目录运行。**

- Docker Compose：`infrastructure/docker-compose.yml`
- 路由配置：`infrastructure/config/api-gateway/routes/`
- nginx：`infrastructure/nginx/`
- 镜像清单：`infrastructure/base-images.json`

**禁止** 使用 `docker/`、`docker-compose.yml`（旧）等其它路径启动服务。容器名固定以 `infrastructure-` 前缀。

## 启动与运行

```powershell
# 1. 启动 infrastructure 基础设施（consul / redis / mysql / api-gateway / token / iam / notification）
cd D:\projects\coder-micro-services\infrastructure
docker compose up -d

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
node scripts/run-e2e-local.js
```

## 环境变量（指向 infrastructure）

| 变量 | 默认 | 说明 |
|---|---|---|
| `E2E_BASE_URL` | `http://localhost:3000` | infrastructure-api-gateway |
| `E2E_ADMIN_USER` | `e2eadmin` | globalSetup 自注册 |
| `E2E_ADMIN_PASS` | `P@ssw0rd!` | globalSetup 自注册 |
| `E2E_DEBUG` | `0` | 1=请求/响应日志 |

## 测试结构

- `globalSetup.ts` 通过 `infrastructure-api-gateway` 注册并登录 e2e admin
- `helpers/` 公共：auth / http / db / ids / debug
- `*.e2e.test.ts` 按服务分文件（auth / users / tokens / roles / ...）

## 调试

```powershell
$env:E2E_DEBUG=1
node scripts/run-e2e-local.js users -t "list users"
```

## AI 操作边界

- 改 `tests/e2e/**` 即可修测试
- **禁止** 改 `services/<svc>/src/`、`libs/`、`sdk/` 业务源码
- 改 `infrastructure/config/api-gateway/routes/` 后必须 `cargo run -p api-gateway -- validate --routes infrastructure\config\api-gateway\routes`