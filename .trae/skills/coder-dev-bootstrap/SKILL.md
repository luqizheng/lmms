---
name: "coder-dev-bootstrap"
description: "首次启动本地开发环境（拉取源码后 / 数据库已清空）。Invoke when 用户说 第一次启动、初始化开发环境、首启、bootstrap、数据库清空后重启。"
---

# coder-dev-bootstrap

## 何时用

- 拉取源码后第一次起本地服务
- 清空数据库表后想重新初始化
- CI 失败想本地复现

## 前置

- 运行环境：WSL / macOS / Linux（Windows 必须在 WSL 内执行）
- Docker 已启动
- Node 18+ / pnpm
- 所有命令在**项目根目录**执行，禁止写死绝对路径

## 目录检查（每次执行前必做）

```bash
pwd
test -d infrastructure && test -f scripts/check-dev-env.js && test -f package.json
```

若不在根目录：`cd <仓库根>`。

## 步骤

### 1. 同步并预拉基础镜像

```bash
pnpm images:sync
```

### 2. 启动基础设施（首次需 build 业务镜像）

```bash
docker compose -f infrastructure/docker-compose.yml up -d --build
```

容器名前缀 `infrastructure-`，关键：
- `infrastructure-mysql-1`
- `infrastructure-redis-1`
- `infrastructure-consul-1`
- `infrastructure-token-service-1`
- `infrastructure-iam-service-1`
- `infrastructure-notification-service-1`
- `infrastructure-api-gateway-1`

### 3. 等待健康

```bash
docker compose -f infrastructure/docker-compose.yml ps
```

等到所有服务为 `running` 或 `healthy`。

### 4. 自动建表说明

清表后首启自动重建，**无需手工执行 SQL**：

- notification-service 启动时通过 `ensure_schema()` 应用 `services/notification-service/migrations/*.sql`
- token-service / iam-service 走各自 `db.rs` 的迁移逻辑

### 5. 环境检查

```bash
pnpm check:env
```

期望：Docker Services 全部 healthy/running，HTTP 服务（gateway/token jwks/iam/notification/prue/consul）全部 OK。

### 6. （可选）冒烟 E2E

自动注册 `e2eadmin / P@ssw0rd!` 并跑认证用例：

```bash
pnpm -C tests/e2e install
node scripts/run-e2e-local.js auth
```

### 7. （可选）前端

- 容器版：`http://localhost:8080`（compose 已含 `frontend` 服务）
- 本地热更：`pnpm -C frontend dev`

## 端口速查

| 服务 | 端口 |
|---|---|
| api-gateway | 3000 |
| token-service | 6001 |
| iam-service | 6002 |
| notification-service | 6003 |
| prue_backend | 6004 |
| consul | 8500 |
| mysql | 3306 |
| redis | 6379 |
| frontend 容器 | 8080 |
| frontend dev | 8849 |

## 故障排查

- 端口冲突：`lsof -i :3000` 或 `ss -lntp | grep :3000`
- 镜像拉不动：`pnpm images:sync` 重试，必要时按 `coder-base-images-sync` skill 设 mirror
- 容器 unhealthy：`docker logs <container>`，优先排查 `infrastructure-mysql-1`
- api-gateway 卡在等 jwks：先看 `infrastructure-token-service-1` 日志
- 彻底重置环境（再次清库）：
  ```bash
  docker compose -f infrastructure/docker-compose.yml down -v
  ```

## 不做的事

- ❌ 不改业务源码
- ❌ 不改 api-gateway routes
- ❌ 不发布镜像
- ❌ 不跑全量 e2e
