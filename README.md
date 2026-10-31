# Coder Micro Services

> **新人入职？** 请先阅读 [`docs-site/onboarding/README.md`](docs-site/onboarding/README.md) — 从零开始的手把手入门指南（中文），覆盖环境准备、第一次跑通、日志排查。

## Overview

A microservices architecture for coder platform, built with Rust.

**本框架不随附前端控制台**（PRISM console 已移除）。前端项目在各自业务仓中，通过 `packages/@digua/contracts` + `@digua/http` + `sdk/iam-services`（npm TS 包，发布到私有 registry `http://192.168.2.12:14873/`）或 `sdk/`（Java / Rust；C# 规划中）接入 api-gateway:3000 唯一公共入口。npm 包的新增与发布规则见 `AGENTS.md`「npm 包发布规则」。

**Full documentation**: [`docs-site/user-manual/`](docs-site/user-manual/) — comprehensive guide covering local dev, testing, observability, deployment, rate limiting, canary rollout, AI workflows, and troubleshooting (拆分章节、中文版).

**AI entry point**: [`docs-site/ai-context.md`](docs-site/ai-context.md) — service map, known pitfalls, digua_common commands (read first when starting work in this repo).

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

# 2. （可选）启动可观测性 overlay（Prometheus + Grafana + Jaeger + OTel collector，
#    已 include 进 docker-compose.yml；想跳过此步可用 `docker compose -f infrastructure/docker-compose-infra.yml up -d`）
docker compose -f infrastructure/docker-compose.yml up -d
# → Prometheus :9090  Grafana :3001  Jaeger :16686

# 3. 安装 e2e 依赖（独立 workspace）
pnpm -C tests/e2e install

# 4. 跑端到端测试（数据驱动 API 用例，直接打 api-gateway:3000）
cd tests/e2e
pnpm test:smoke        # 2 min smoke 子集
pnpm test:api          # 5 min 全量 API

# 5. 停止基础设施
docker compose -f infrastructure/docker-compose.yml down
```

### 常用测试命令

| 命令                              | 说明                                              |
| --------------------------------- | ------------------------------------------------- |
| `pnpm run test:e2e`               | 全量 e2e（Vitest + supertest，跨网关测真服务）    |
| `pnpm run test:e2e:api`           | 数据驱动 API 用例（Excel）                        |
| `pnpm run test:e2e:smoke`         | E2E smoke 子集（2 min，仅 API）                   |
| `pnpm run test:rust`              | 容器内跑 Rust 单元测试（可选 `[service]` 参数；底层 `scripts/run-tests-docker.js`） |
| `pnpm run test:all`               | 一键跑 workspace + Rust + e2e + js 脚本 + sdk 全套 |
| `pnpm run check:env` | 检查开发环境服务状态 |
| `pnpm -C tests/e2e test:smoke` | E2E smoke 子集（2 min，仅 API 用例） |
| `node scripts/check-service-standard.js` | CI gate: 8-item 服务规范检查 |
| `node scripts/check-spec.js` | CI gate: OpenAPI / 错误码 / 权限码检查 |
| `node scripts/contracts-gen.js --check` | CI gate: 契约生成代码与 YAML 一致性 |
| `node scripts/dump-openapi-specs.js` | 重新生成 4 个 service 的 openapi.yaml |

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

| 服务                 | compose host 端口 | `cargo run` 默认 | 依赖                                      | OTel |
| -------------------- | ------------------ | ---------------- | ----------------------------------------- | ---- |
| api-gateway          | 3000               | 3000             | token-service, iam-service（consul 发现） | ✅ |
| token-service        | 8082               | 4000¹            | MySQL / Redis / Consul                    | ✅ |
| iam-service          | 9081²              | 8081             | MySQL / Consul                            | ✅ |
| notification-service | 8083               | 8083             | MySQL / Redis / Consul                    | ✅ |
| prue_backend         | 8087               | 8080¹            | SQLite / MySQL / Consul                   | ✅ |
| audit-service        | 8084               | 8080¹            | MySQL / Consul                            | ✅ |
| config-service       | 8085³              | 8081             | MySQL / Consul                            | ✅ |
| file-service         | 8086³              | 8082             | MySQL / Consul                            | ✅ |
| consul               | 8500               | —                | —                                         | — |
| redis                | 6379               | —                | —                                         | — |
| mysql                | 3306               | —                | root / rootpass                           | — |

> ¹ **token-service / audit-service / prue_backend 的 `cargo run` 默认端口**与 compose 不一致——code default 是 8080（audit / prue_backend）或 4000（token-service），compose 显式 `SERVER_PORT=...` 覆盖。开发时直跑 cargo 不带 env 会撞 8080 / 4000；带 compose env 后才落到 808x。
>
> ² **iam-service compose host 端口 9081** 是 Phase 5-A mesh 时代 sidecar 外端口残留（容器内一直是 8081），spec §7.3 规划统一到 `8081:8081`，待落地。
>
> ³ **config-service / file-service 的 host 端口 8085 / 8086** 是 Phase 5-A-2 引入的消歧（容器内与 iam / token 同端口，靠 host 端口区分）。
>
> 跨服务调用走 **容器内端口**（`http://iam-service:8081` 等），由 Docker `superpowers` 网络 DNS 解析；host 端口只用于宿主机 `curl` 开发联调，gateway 不走 host 端口。详见 [`docs-site/user-manual/11-reference.md` §11.5](docs-site/user-manual/11-reference.md#115-服务端口速查phase-5-a-2-之后) 与 [`docs-site/user-manual/02-architecture.md` §2.5](docs-site/user-manual/02-architecture.md#25-swagger-ui-覆盖)。
>
> ✅ = 启用 OpenTelemetry traces（跨服务 trace 链在 Jaeger 可视化）

### 可观测性

详见 [`docs-site/guides/observability.md`](docs-site/guides/observability.md)。

```bash
docker compose -f infrastructure/docker-compose.yml up -d
# Prometheus :9090  (metrics scraping, Consul SD)
# Grafana    :3001  (admin/admin, 预置 Gateway Overview dashboard)
# Jaeger     :16686 (traces UI, 4 OTel-enabled services)
# OTel collector OTLP gRPC :4317 (接收 service spans)
```

### Swagger UI

设置环境变量 `SWAGGER_ENABLED=true` 启动服务后，可访问以下地址查看 API 文档（端口为 compose host 端口；`cargo run` 本地默认见上表）：

| 服务                 | compose host 端口 | Swagger UI                       | OpenAPI JSON                                |
| -------------------- | ------------------ | -------------------------------- | ------------------------------------------- |
| token-service        | 8082               | http://localhost:8082/swagger-ui  | http://localhost:8082/api-docs/openapi.json |
| iam-service          | 9081               | http://localhost:9081/swagger-ui  | http://localhost:9081/api-docs/openapi.json |
| notification-service | 8083               | http://localhost:8083/swagger-ui  | http://localhost:8083/api-docs/openapi.json |
| prue_backend         | 8087               | http://localhost:8087/swagger-ui  | http://localhost:8087/api-docs/openapi.json |
| audit-service        | 8084               | http://localhost:8084/swagger-ui  | http://localhost:8084/api-docs/openapi.json |
| config-service       | 8085               | http://localhost:8085/swagger-ui  | http://localhost:8085/api-docs/openapi.json |
| file-service         | 8086               | http://localhost:8086/swagger-ui  | http://localhost:8086/api-docs/openapi.json |

> 全部 7 个业务服务的 openapi.yaml 都是 utoipa 自动生成的。`api-gateway` 是代理，无自有路由（详情见 `services/api-gateway/openapi.yaml`）。详见 `node scripts/dump-openapi-specs.js` 和 `.agents/skills/coder-new-service`。

### API Gateway 限流

api-gateway 集中限流，按 `infrastructure/config/api-gateway/rate-limits.toml` 配置：

```toml
[defaults]
requests_per_second = 100
burst = 200

[routes."/v1/iam/auth/login"]
requests_per_second = 5     # auth 端点严格
burst = 10
```

限流时返回 HTTP 429 + `Retry-After: 1`。详见 [`docs-site/user-manual/07-rate-limiting.md`](docs-site/user-manual/07-rate-limiting.md)。

### 灰度发布

详见 [`docs-site/guides/canary-rollout.md`](docs-site/guides/canary-rollout.md)。

```bash
# 部署 canary 实例 + tag
docker exec consul consul tags -add -service audit-service -tag canary -id <id>

# 10% 流量到 canary；用 X-Canary: true 强制路由

# 回滚（1 秒内）
docker exec consul consul tags -delete -service audit-service -tag canary -id <id>
```

### 配置文件位置

服务进程读取以下配置文件：

- `services/<svc>/config/config.yaml`（宿主机路径；容器化部署时由 `infrastructure/docker-compose-services.yml` bind mount 进 `/app/config:ro`）

基础设施的 docker compose 文件位于 `infrastructure/`：

- `docker-compose.yml` — 仅基础设施（3 容器：consul / redis / mysql，配合本地服务进程）
- `docker-compose-observability.yml` — overlay（Prometheus + Grafana + Jaeger + OTel collector）

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
- **trace 不出现在 Jaeger**：检查 `OTEL_EXPORTER_OTLP_ENDPOINT` 环境变量是否设置（详见 `docs-site/guides/otel-tracing.md`）
- **API 返回 429**：被 rate limit 了，调整 `infrastructure/config/api-gateway/rate-limits.toml`
- **OpenAPI drift CI 失败**：运行 `node scripts/dump-openapi-specs.js` 重新生成并 commit

## Services

| Service              | Language | Framework | OTel |
| -------------------- | -------- | --------- | ---- |
| API Gateway          | Rust     | Axum      | ✅ |
| Token Service        | Rust     | Axum      | ✅ |
| IAM Service          | Rust     | Axum      | ✅ |
| Notification Service | Rust     | Axum      | ✅ |
| Audit Service        | Rust     | Axum      | — |
| Config Service       | Rust     | Axum      | — |
| File Service         | Rust     | Axum      | — |

> 旧 `auth-service` / `user-service` 已废弃并合并进 `iam-service`。

## AI 辅助开发

项目使用 `.agents/skills/` 提供 AI skill：

| Skill | 用途 |
|-------|------|
| `coder-add-route` | 给现有 service 加路由（含 utoipa 注解 + openapi.rs 注册） |
| `coder-migrate-db` | 加迁移（MySQL + SQLite 双版本） |
| `coder-new-service` | 脚手架新 Rust microservice（满足 8-item 服务规范） |
| `coder-sdk-publish-rust` | 发布 Rust crates 到 kellnr 私有 registry（libs/* 17 + sdk/rust/* 7，含拓扑顺序与 dry-run） |
| `coder-sdk-publish-maven` | 发布 Java SDK 到 Reposilite（release + snapshot，含 `mvn versions:set` 自动 bump + curl 上传模式 + 4 级凭证 fallback） |
| `coder-service-release` | 服务镜像 release（`docker:build` + `release:build` + `release:deploy`，含 compose `command:` 命名扫描与 L1/L2 loop 校验） |

完整 L4 触发器规则见 `.trae/rules/project_rules.md`。

## CI/CD Deployment

### SSH Key Setup

To enable automated deployment via GitLab CI, follow these steps:

#### Step 1: Generate SSH Key Pair on Deployment Server

Login to your deployment server (replace `<deploy-server-ip>` with actual IP):

```bash
ssh <deploy-user>@<deploy-server-ip>
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
| `DEPLOY_SERVER`   | `<deploy-server-ip>` (set in GitLab CI/CD vars)    | Yes       |
| `DEPLOY_USER`     | `<deploy-username>` (set in GitLab CI/CD vars)     | Yes       |
| `DEPLOY_PASSWORD` | `<deploy-password>` (set in GitLab CI/CD vars)     | Yes       |
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
ssh <deploy-user>@<deploy-server-ip>

# Navigate to project directory
cd /opt/coder-micro-services

# Pull latest changes
git pull origin main

# Stop existing containers
docker-compose -f infrastructure/docker-compose.yml down

# Pull latest images
docker-compose -f infrastructure/docker-compose.yml pull

# Start containers in detached mode
docker-compose -f infrastructure/docker-compose.yml up -d
```

## 离线分发包（私有化部署）

项目提供完整的**离线分发包**，适用于无法访问外网的私有化部署环境。

### 核心文件

| 文件 | 说明 |
|------|------|
| [`release/dist-template/`](release/dist-template/) | 离线分发包完整目录 |
| [`release/dist-template/README.md`](release/dist-template/README.md) | 分发包使用文档 |
| [`release/dist-template/docs/`](release/dist-template/docs/) | 完整部署文档（含故障排查、升级、备份） |
| `release/dist-template/index.html` | 文档入口（浏览器直接打开，内置搜索） |

### 快速部署

```bash
# 1. 加载 Docker 镜像（约 10-15GB）
node bin/load-images.js

# 2. 生成密钥和密码
make secrets

# 3. 启动所有服务
make up

# 4. 验证
make verify
```

### 离线镜像打包（在有网环境执行）

```bash
# 打包离线镜像（输出 release/coder-micro-services-v1.0.0-airgap.tar.gz）
node release/dist-template/scripts/release.js v1.0.0
```

> 路径注意：脚本实际位于 `release/dist-template/scripts/release.js`，本节必须带 `release/dist-template/` 前缀。
>
> 可选参数：`--no-images` 跳过 `docker pull/save`（仅打配置/骨架包，需消费方自备镜像），等价于 `EXPORT_IMAGES=0 node release/dist-template/scripts/release.js v1.0.0`。详见 [`release/dist-template/README.md`](release/dist-template/README.md) §「离线镜像打包 / 选项」。
>
> 详见 [`release/dist-template/docs/deployment/air-gap.md`](release/dist-template/docs/deployment/air-gap.md)

## Infrastructure

- **Consul**: Service discovery
- **Redis**: Token storage, caching
- **MySQL**: Persistent data storage

## Configuration

Configuration files are located in `config/` directory:

- `api-gateway/api-gateway.toml` - API Gateway global settings (server / redis / jwks / consul)
- `api-gateway/routes/*.router.toml` - API Gateway routing rules, one file per URL prefix
- `token-service.toml` - Token Service settings

API Gateway routing is split per URL prefix; each `routes/<name>.router.toml` declares its `match` pattern, target `service`, `public` flag and optional `rewrite` rule chain. The gateway exposes a `validate` subcommand for CI checks and a `serve` subcommand with `SIGHUP` / `POST /__admin/reload` for hot reload. See `docs-site/guides/api-gateway-routing.md` for the full usage manual and `docs-site/decisions/ADR-001-api-gateway-multi-file-router-config.md` for rationale.

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
| `pnpm run docker:build`                      | 构建所有服务镜像（交互式；支持 `-v`/`--push`/`--load-and-push`/`--platform`，见「常用工作流」示例） |
| `pnpm run docker:build:api-gateway`          | 仅构建 api-gateway 镜像                           |
| `pnpm run docker:build:token-service`        | 仅构建 token-service 镜像                         |
| `pnpm run docker:build:iam-service`          | 仅构建 iam-service 镜像                           |
| `pnpm run docker:build:notification-service` | 仅构建 notification-service 镜像                  |
| `pnpm run docker:build:audit-service`        | 仅构建 audit-service 镜像                         |
| `pnpm run docker:build:config-service`       | 仅构建 config-service 镜像                        |
| `pnpm run docker:build:file-service`         | 仅构建 file-service 镜像                          |
| `pnpm run check:env`                          | 检查开发环境服务状态                              |
| `pnpm run ai:review`                          | AI 代码审查，自动识别不符合规范的代码             |
| `pnpm run ai:gen-e2e`                         | 从 OpenAPI 文档自动生成 E2E 测试用例             |
| `pnpm run contracts:gen`                      | 生成契约代码（错误码、权限码）                    |
| `pnpm run check:service-standard`             | 检查服务规范合规性                                |
| `pnpm run check:spec`                         | 检查契约规范（权限码、错误码、OpenAPI）           |
| `node scripts/dump-openapi-specs.js`          | 重新生成 4 个 service 的 openapi.yaml            |
| `docker compose -f infrastructure/docker-compose.yml up -d` | 启动全部基础设施 + 业务 + 可观测性（observability 已 include 进主 compose） |

### AI 辅助工具

#### 智能代码审查

自动扫描代码库，识别不符合规范的代码：

```bash
# 扫描整个仓库
pnpm run ai:review

# 扫描指定目录
pnpm run ai:review services/

# 检查内容包括：
# - Rust: 命名规范(snake_case)、未使用导入、panic!宏使用
# - JavaScript/TypeScript: var声明、console.log使用
```

#### 测试用例生成

从 OpenAPI 文档自动生成 E2E 测试用例：

```bash
# 从 OpenAPI 文档生成测试用例
pnpm run ai:gen-e2e <openapi文件> <输出路径>

# 示例
pnpm run ai:gen-e2e services/audit-service/openapi.yaml tests/e2e/api/cases/audit-auto.yaml
```

#### 契约代码生成

管理全局错误码和权限码的单一真相源：

```bash
# 生成契约代码
pnpm run contracts:gen

# CI 检查契约是否更新
pnpm run contracts:check
```

#### 契约规范检查

验证权限码、错误码和 OpenAPI 文档的规范性：

```bash
# 检查所有契约规范
pnpm run check:spec

# 检查内容包括：
# - 权限码：格式是否正确（SERVICE_RESOURCE_ACTION）、是否重复
# - 错误码：格式是否正确（SERVICE_CATEGORY_NUMBER）、是否重复
# - OpenAPI：必填字段是否齐全（operationId、summary 等）
```

### 子包脚本

```bash
# token-service (Rust)
pnpm --filter token-service run build
pnpm --filter token-service run test
pnpm --filter token-service run run

# packages/（前端 TS 集成包：@digua/contracts + @digua/http）
pnpm run packages:build      # 构建两个包
pnpm run packages:check      # 发布前校验
pnpm run packages:publish    # 发布到私有 registry npm.kudi.repos
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
pnpm run docker:build                                   # 交互式（默认推送 Y / linux/amd64）
pnpm run docker:build -- -v 1.0.0 --push                # 参数式：版本号 1.0.0，直接推送
pnpm run docker:build -- --load-and-push -v 1.0.0       # 本地留镜像 + 推送
pnpm run docker:build -- --platform linux/amd64,linux/arm64 --push   # 多架构
```

## 参考文档

按使用场景：

| 场景 | 文档 |
|------|------|
| 我是新人 / 新 AI 实例 | [`docs-site/user-manual/`](docs-site/user-manual/) |
| AI 协作者读取入口 | [`docs-site/ai-context.md`](docs-site/ai-context.md) |
| 加路由 / 加迁移 / 新建 service | [`.agents/skills/`](.agents/skills/) |
| E2E 测试 (smoke / data-driven) | [`docs-site/guides/testing.md`](docs-site/guides/testing.md) |
| OpenTelemetry / Jaeger / 跨服务 trace | [`docs-site/guides/otel-tracing.md`](docs-site/guides/otel-tracing.md) |
| 可观测性 stack (Prometheus + Grafana + OTel) | [`docs-site/guides/observability.md`](docs-site/guides/observability.md) |
| Canary 灰度发布 | [`docs-site/guides/canary-rollout.md`](docs-site/guides/canary-rollout.md) |
| api-gateway 路由配置 | [`docs-site/guides/api-gateway-routing.md`](docs-site/guides/api-gateway-routing.md) |
| ADR (架构决策) | [`docs-site/decisions/`](docs-site/decisions/) |
| 12 周路线图与月度复盘 | [`docs/superpowers/`](docs/superpowers/) |
| **私有化离线部署** | [`release/dist-template/`](release/dist-template/) |
| 离线部署步骤 | [`release/dist-template/docs/deployment/air-gap.md`](release/dist-template/docs/deployment/air-gap.md) |
