---
name: coder-microservice-dev
description: 启动 coder-micro-services 本地开发环境（4 服务 + MySQL/Redis/Consul），便于热重载与断点调试。用于「起服务」「本地开发」「dev」等意图。
---

# coder-microservice-dev

## 何时用

- 改业务代码后要本地试
- 断点调试某个 service
- 用户说「起服务」「跑起来」「本地跑」时

## 模式

**基础设施跑在 Docker，服务进程跑在宿主机**（便于 cargo run / go run 热重载）。

## 步骤

### 1. 启动基础设施

```powershell
cd d:\projects\coder-micro-services
pnpm run dev:infra
```

= `docker compose -f infrastructure/docker-compose.dev.yaml up -d`

### 2. 启动 4 个服务（4 个独立终端）

```powershell
# 终端 1: api-gateway
cd services\api-gateway
cargo run -- serve --routes docker\config\api-gateway\routes

# 终端 2: token-server
cd services\token-server
cargo run

# 终端 3: auth-service
cd services\auth-service
go run .

# 终端 4: user-service
cd services\user-service
go run .
```

或并行：

```powershell
pnpm run dev
```

### 3. 端口

| 服务 | 端口 |
|------|------|
| api-gateway | 7000（生产 3000） |
| token-server | 4000 |
| auth-service | 5000 |
| user-service | 6000 |
| consul | 8500 |
| redis | 6379 |
| mysql | 3306 (root/rootpass) |

### 4. 烟测

```bash
curl -i http://localhost:7000/health
```

### 5. 前端（可选）

```bash
cd frontend
pnpm run dev
```

### 6. 停止

```powershell
docker compose -f infrastructure\docker-compose.dev.yaml down
```

## 配置优先级

服务读 `services/<svc>/config/config.yaml`（首选，宿主机路径） → `docker/config/<svc>/config.yaml`（容器化部署）。

## 故障

- MySQL 容器找不到：`docker ps -a` 看 `coder-micro-services` 前缀
- cargo 链接错误（fiat_25519）：仓库已切到 `aws_lc_rs`，不要回退 `ring`
- 服务起不来：先 `docker logs <container>` 看基础设施
