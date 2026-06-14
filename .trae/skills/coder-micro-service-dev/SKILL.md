---
name: coder-microservice-dev
description: 启动 coder-micro-services 本地开发环境（基础设施 + 服务），便于热重载与断点调试。用于「起服务」「本地开发」「dev」等意图。
---

# coder-micro-service-dev

## 何时用

- 改业务代码后要本地测试
- 断点调试某个 service
- 用户说「起服务」「跑起来」「本地跑」时

## 模式

**基础设施跑在 Docker，服务进程跑在宿主机**（便于 cargo run 热重载）。

## 步骤

### 1. 启动基础设施（Docker）

```powershell
cd d:\projects\coder-micro-services
docker compose -f infrastructure/docker-compose.yml up -d
```

### 2. 启动服务（独立终端）

```powershell
# 终端 1: api-gateway
cd services\api-gateway
cargo run -- serve --routes docker\config\api-gateway\routes

# 终端 2: token-service
cd services\token-service
cargo run

# 终端 3: iam-service
cd services\iam-service
cargo run

# 终端 4: notification-service
cd services\notification-service
cargo run
```

### 3. 端口

| 服务 | 外部端口 | 内部端口 |
|------|----------|----------|
| api-gateway | 3000 | 3000 |
| token-service | 6001 | 8080 |
| iam-service | 6002 | 8080 |
| notification-service | 6003 | 8080 |
| consul | 8500 | 8500 |
| redis | 6379 | 6379 |
| mysql | 3306 | 3306 (root/rootpass) |

### 4. 烟测

```bash
curl -i http://localhost:3000/health
```

### 5. 停止

```powershell
docker compose -f infrastructure/docker-compose.yml down
```

## 配置优先级

服务读 `services/<svc>/config/config.yaml`（首选，宿主机路径） → `docker/config/<svc>/config.yaml`（容器化部署）。

## 配置目录

路由配置：`docker/config/api-gateway/routes/`
服务配置：`docker/config/<svc>/`

## 故障

- MySQL 容器找不到：`docker ps -a` 看 `coder-micro-services` 前缀
- cargo 链接错误（fiat_25519）：仓库已切到 `aws_lc_rs`，不要回退 `ring`
- 服务起不来：先 `docker logs <container>` 看基础设施

## 注意

- `user-service` 已废弃，功能已迁移到 `iam-service`
- 所有服务均为 Rust 项目，使用 `cargo run` 启动
- 服务内部端口统一为 8080，对外端口范围 6000-6999
- api-gateway 对外对内端口均保持 3000 不变