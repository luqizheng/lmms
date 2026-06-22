# Coder Micro Services

## Overview

A microservices architecture for coder platform, built with Rust.

## 快速开始（本地开发与测试）

本地采用 **「基础设施跑在 Docker、服务进程跑在宿主机」** 的开发模式：MySQL / Redis / Consul 通过 `docker compose` 启动，各服务（api-gateway、token-service、iam-service、notification-service、audit-service、config-service、file-service）直接以 `cargo run` 跑在宿主机上，便于热重载和断点调试。

### 前置依赖

- Docker Desktop（Windows 启用 WSL2 backend）
- Rust 工具链（`rustup` + `cargo`，1.96+）
- Node.js 18+（跑 `scripts/*.js` 编排器）

### 一键工作流

```bash
# 1. 启动基础设施 + 服务（infrastructure/docker-compose.yml）
docker compose -f infrastructure/docker-compose.yml up -d

# 2. 安装 e2e 依赖（独立 workspace）
pnpm -C tests/e2e install

# 3. 跑端到端测试（详见 tests/e2e/README.md）
pnpm run test:e2e

# 4. 停止基础设施
docker compose -f infrastructure/docker-compose.yml down
```

### 常用测试命令

| 命令                              | 说明                                              |
| --------------------------------- | ------------------------------------------------- |
| `pnpm run test:e2e`               | 全量 e2e（Vitest + supertest，跨网关测真服务）    |
| `pnpm run test:e2e:users` 等      | 按服务分组跑（auth/users/tokens/roles/... 见 package.json） |
| `pnpm run test:e2e:api`           | 数据驱动 API 用例（Excel）                        |
| `pnpm run test:e2e:ui`            | 数据驱动 UI 业务流（Playwright，需前端容器）      |
| `node scripts/run-tests-docker.js [service]` | 容器内跑 Rust 单元测试（见 AGENTS.md） |
| `pnpm run check:env` | 检查开发环境服务状态 |

### 单独跑某个服务

```bash
# Rust 服务
cd services\api-gateway
cargo test --lib                 # 单元测试
cargo test --test integration_test # 集成测试
cargo run -- serve               # 启动服务（默认读 config\config.toml）

# 其他 Rust 服务（iam-service / notification-service / audit-service / config-service / file-service / token-service）
cd services\iam-service
cargo test                       # 测试
cargo run                        # 启动
```

### 服务端口与依赖

| 服务                 | 外部端口 | 依赖                                      |
| -------------------- | -------- | ----------------------------------------- |
| api-gateway          | 3000     | token-service, iam-service（consul 发现） |
| token-service        | 6001     | MySQL / Redis / Consul                    |
| iam-service          | 6002     | MySQL / Consul                            |
| notification-service | 6003     | MySQL / Redis / Consul                    |
| audit-service        | —        | MySQL / Consul                            |
| config-service       | —        | MySQL / Consul                            |
| file-service         | —        | MySQL / Consul                            |
| consul               | 8500     | —                                         |
| redis                | 6379     | —                                         |
| mysql                | 3306     | root / rootpass                           |

> 服务内部端口统一为 8080，对外端口范围 6000-6999；api-gateway 对外/对内均为 3000。

### Swagger UI

设置环境变量 `SWAGGER_ENABLED=true` 启动服务后，可访问以下地址查看 API 文档：

| 服务          | Swagger UI                       | OpenAPI JSON                                |
| ------------- | -------------------------------- | ------------------------------------------- |
| token-service | http://localhost:6001/swagger-ui | http://localhost:6001/api-docs/openapi.json |
| iam-service   | http://localhost:6002/swagger-ui | http://localhost:6002/api-docs/openapi.json |

### 配置文件位置

服务进程读取以下配置文件（优先级从高到低）：

- `services/<svc>/config/config.yaml`（首选，宿主机路径）
- `docker-example/config/<svc>/config.yaml`（容器化部署时使用）

基础设施的 docker compose 文件位于 `infrastructure/`：

- `docker-compose.yml` — 仅基础设施（3 容器：consul / redis / mysql，配合本地服务进程）

### 在 Docker 中跑 Rust 单元测试

采用 **volume mapping 模型**：不构建镜像，直接把源码与 cargo 缓存挂载进基础镜像内执行 `cargo test`，与生产构建一致使用 `x86_64-unknown-linux-musl` 目标。

```powershell
# 全部服务
node scripts/run-tests-docker.js

# 指定一个或多个服务
node scripts/run-tests-docker.js iam-service token-service
```

等价的底层 docker 命令：

```powershell
docker run --rm `
  --network host `
  -v ${PWD}:/app `
  -v coder-cargo-registry:/usr/local/cargo/registry `
  -v coder-cargo-git:/usr/local/cargo/git `
  -v coder-cargo-target:/app/target `
  -v /var/run/docker.sock:/var/run/docker.sock `
  -w /app `
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl `
  cargo test --target x86_64-unknown-linux-musl -p iam-service -- --nocapture
```

要点：

- 基础镜像复用 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`，无需 `Dockerfile.test`
- 源码用 bind mount 挂载，改代码**零 rebuild**
- `registry` / `git` / `target` 用命名 volume 持久化，跨 service、跨次复用增量编译
- `--network host`：部分 build.rs（如 `utoipa-swagger-ui`）需联网下载资源
- 使用 testcontainers 的服务（token-service / iam-service 等）必须挂载 `/var/run/docker.sock`
- 范围限定 `-p ${SERVICE}`，仅跑服务自身测试，不含 libs / sdk
- 多服务共享同一 `target` volume，脚本按串行执行避免 target 锁争用
- 依赖 TLS 时统一用 workspace 的 `reqwest 0.12 + rustls`，禁用 native-tls（musl 镜像无 openssl headers）

### 故障排查

- **MySQL 容器找不到**：`docker ps -a` 看容器名；`start` 启动的容器会自动加 `coder-micro-services` 前缀
- **cargo 链接错误（fiat_25519）**：本仓库已切到 `aws_lc_rs` crypto provider；如回退到 `ring` 会触发 MSVC LNK2019
- **服务起不来**：`docker logs <container>` 看基础设施；服务进程日志直接输出到终端
- **测试数据脏**：e2e 的 `globalSetup` 会在每轮开始清库重置；也可重启 mysql 容器

## Services

| Service              | Language | Framework |
| -------------------- | -------- | --------- |
| API Gateway          | Rust     | Axum      |
| Token Service        | Rust     | Axum      |
| IAM Service          | Rust     | Axum      |
| Notification Service | Rust     | Axum      |
| Audit Service        | Rust     | Axum      |
| Config Service       | Rust     | Axum      |
| File Service         | Rust     | Axum      |

> 旧 `auth-service` / `user-service` 已废弃并合并进 `iam-service`。

## CI/CD Deployment

### SSH Key Setup

To enable automated deployment via GitLab CI, follow these steps:

#### Step 1: Generate SSH Key Pair on Deployment Server

Login to your deployment server (`192.168.2.128`):

```bash
ssh kudi@192.168.2.128
```

Generate an ED25519 key pair (no passphrase):

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_rsa -N ""
```

#### Step 2: Configure SSH Authorization

Add the public key to authorized_keys:

```bash
cat ~/.ssh/id_rsa.pub >> ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys
chmod 700 ~/.ssh
```

#### Step 3: Retrieve Private Key

Copy the private key content:

```bash
cat ~/.ssh/id_rsa
```

#### Step 4: Configure GitLab CI Variables

1. Go to your GitLab project
2. Navigate to **Settings** > **CI/CD** > **Variables**
3. Add the following variables:

| Variable          | Value                                              | Protected |
| ----------------- | -------------------------------------------------- | --------- |
| `SSH_PRIVATE_KEY` | Content of id_rsa file                             | Yes       |
| `DEPLOY_SERVER`   | `192.168.2.128`                                    | Yes       |
| `DEPLOY_USER`     | `admin`                                            | Yes       |
| `DEPLOY_PASSWORD` | Server admin password                              | Yes       |
| `DOCKER_REGISTRY` | Docker registry URL (e.g., `registry.example.com`) | Yes       |

### Deployment Pipeline

The CI/CD pipeline consists of three stages:

1. **test**: Run Rust tests
2. **build**: Build and push Docker images
3. **deploy**: Deploy to production server

### Manual Deployment

If manual deployment is needed:

```bash
# SSH to deployment server
ssh kudi@192.168.2.128

# Navigate to project directory
cd /opt/coder-micro-services

# Pull latest changes
git pull origin master

# Stop existing containers
docker-compose -f infrastructure/docker-compose.yml down

# Pull latest images
docker-compose -f infrastructure/docker-compose.yml pull

# Start containers in detached mode
docker-compose -f infrastructure/docker-compose.yml up -d
```

## Infrastructure

- **Consul**: Service discovery
- **Redis**: Token storage, caching
- **MySQL**: Persistent data storage

## Configuration

Configuration files are located in `config/` directory:

- `api-gateway/api-gateway.toml` - API Gateway global settings (server / redis / jwks / consul)
- `api-gateway/routes/*.router.toml` - API Gateway routing rules, one file per URL prefix
- `token-service.toml` - Token Service settings

API Gateway routing is split per URL prefix; each `routes/<name>.router.toml` declares its `match` pattern, target `service`, `public` flag and optional `rewrite` rule chain. The gateway exposes a `validate` subcommand for CI checks and a `serve` subcommand with `SIGHUP` / `POST /__admin/reload` for hot reload. See `docs/guides/api-gateway-routing.md` for the full usage manual and `docs/decisions/ADR-001-api-gateway-multi-file-router-config.md` for rationale.

## NPM Scripts

项目使用 pnpm workspace 管理，通过根目录的 `package.json` 统一调度。

### 前置依赖

```bash
# 安装 pnpm
npm install -g pnpm

# 安装工作区依赖
pnpm install

# Windows: 准备 NASM (ring 0.17.x 编译时需要)
pnpm run setup
```

### 根目录脚本

| 脚本                                         | 作用                                              |
| -------------------------------------------- | ------------------------------------------------- |
| `pnpm run build`                             | 编译 api-gateway 与 token-service（Rust release） |
| `pnpm run dev`                               | 启动基础设施并并行运行各服务 dev 任务             |
| `pnpm run dev:infra`                         | 仅启动 Docker 基础设施（Consul / Redis / MySQL）  |
| `pnpm run check`                             | 在所有 workspace 包中执行 `check`                 |
| `pnpm run test`                              | 在所有 workspace 包中执行 `test`                  |
| `pnpm run clean`                             | 在所有 workspace 包中执行 `clean`                 |
| `pnpm run docker:build`                      | 构建所有服务 Docker 镜像                          |
| `pnpm run docker:build:api-gateway`          | 仅构建 api-gateway 镜像                           |
| `pnpm run docker:build:token-service`        | 仅构建 token-service 镜像                         |
| `pnpm run docker:build:iam-service`          | 仅构建 iam-service 镜像                           |
| `pnpm run docker:build:notification-service` | 仅构建 notification-service 镜像                  |
| `pnpm run docker:build:audit-service`        | 仅构建 audit-service 镜像                         |
| `pnpm run docker:build:config-service`       | 仅构建 config-service 镜像                        |
| `pnpm run docker:build:file-service`         | 仅构建 file-service 镜像                          |
| `pnpm run check:env`                          | 检查开发环境服务状态                              |

### 子包脚本

```bash
# token-service (Rust)
pnpm --filter token-service run build
pnpm --filter token-service run test
pnpm --filter token-service run run

# frontend
pnpm --filter @coder/frontend run dev
pnpm --filter @coder/frontend run build
```

### 常用工作流

```bash
# 本地开发
pnpm run dev:infra
pnpm run dev

# 单独构建某个服务
pnpm --filter token-service run build

# 完整 CI 流程
pnpm run check && pnpm run test && pnpm run build

# 构建镜像
pnpm run docker:build
```
