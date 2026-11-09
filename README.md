# Coder Micro Services

> **新人入职？** 请先阅读 [`docs-site/onboarding/README.md`](docs-site/onboarding/README.md) — 从零开始的手把手入门指南（中文），覆盖环境准备、第一次跑通、日志排查。

## Overview

A microservices architecture for coder platform, built with Rust.

**本框架不随附前端控制台**（PRISM console 已移除）。前端项目在各自业务仓中，通过 `packages/@digua/contracts` + `@digua/http` + `sdk/{iam,audit,config,file,notification,prue}-services`（npm TS 包，发布到私有 registry `http://192.168.2.12:14873/`）或 `sdk/`（Java / Rust；C# 规划中）接入 api-gateway:3000 唯一公共入口。npm 包的新增与发布规则见 `AGENTS.md`「npm 包发布规则」。

**Full documentation**: [`docs-site/user-manual/`](docs-site/user-manual/) — comprehensive guide covering local dev, testing, observability, deployment, rate limiting, canary rollout, AI workflows, and troubleshooting (拆分章节、中文版).

**AI entry point**: [`docs-site/ai-context.md`](docs-site/ai-context.md) — service map, known pitfalls, digua_common commands (read first when starting work in this repo).

## 快速开始（本地开发与测试）

本地采用 **「基础设施跑在 Docker、服务进程跑在宿主机」** 的开发模式：MySQL / Redis / Consul 通过 `docker compose` 启动，各服务（api-gateway、token-service、iam-service、notification-service、audit-service、file-service、prue_backend）直接以 `cargo run` 跑在宿主机上，便于热重载和断点调试。

### 前置依赖

- Docker Desktop
- Rust 工具链（`rustup` + `cargo`，1.96+）；Windows 另需 Visual Studio Build Tools（C 依赖 `ring` / `aws-lc-sys` 需要 `cl.exe`）
- Node.js 18+（跑 `scripts/*.js` 编排器）

### 一键工作流

```bash
# 1. 启动基础设施（pg 方言；mysql 用 dev:infra:mysql）
pnpm run dev:infra:pg          # infra-postgres/docker-compose.yml（consul/redis/rabbitmq/postgres）
pnpm run dev:pg                # 起基础设施 + 并行跑各服务 dev 任务

# 2. （可选）可观测性：prometheus / grafana / jaeger / otel-collector
docker compose -f infrastructure/observability/docker-compose-observability.yml up -d
# → Prometheus :9090  Grafana :3001  Jaeger :16686

# 3. 安装 e2e 依赖（独立 workspace）
pnpm -C tests/e2e install

# 4. 跑端到端测试（数据驱动 API 用例，直接打 api-gateway:3000）
cd tests/e2e
pnpm test:smoke        # 2 min smoke 子集
pnpm test:api          # 5 min 全量 API

# 5. 停止基础设施
docker compose -f infrastructure/infra-postgres/docker-compose.yml down
```

> 想一条命令起**全栈**（infra + observability + services）用示例部署单元：
> `cd examples/docker-deploy-full && cp .env.example .env && ./start.sh`
> （见「[部署与发布](#部署与发布)」）。

### 常用测试命令

| 命令                              | 说明                                              |
| --------------------------------- | ------------------------------------------------- |
| `cargo test --workspace --lib`    | Rust 单元测试（本机；容器内跑见 `docs-site/ai-context.md`） |
| `pnpm run test`                   | 所有 workspace 包的 test 任务                      |
| `pnpm -C tests/e2e test:smoke`    | E2E smoke 子集（2 min，仅 API 用例）               |
| `pnpm -C tests/e2e test:api`      | 数据驱动 API 用例（Excel 全量）                    |
| `pnpm run test:scripts`           | 脚本层单测（命名规范 / 迁移 / 镜像钉版）           |
| `pnpm run test:cli`               | `packages/cli` 单测                                |
| `pnpm run check:env`              | 检查开发环境服务状态                               |
| `node scripts/check-service-standard.js` | CI gate: 8-item 服务规范检查                |
| `node scripts/check-spec.js`      | CI gate: OpenAPI / 错误码 / 权限码检查             |
| `node scripts/contracts-gen.js --check` | CI gate: 契约生成代码与 YAML 一致性          |
| `node scripts/update-image-env.js --check` | CI gate: `.image.env` 与服务版本一致        |

### 单独跑某个服务

```bash
# Rust 服务
cd services\api-gateway
cargo test --lib                 # 单元测试
cargo test --test integration_test # 集成测试
cargo run -- serve               # 启动服务（默认读 config\config.toml）

# 其他 Rust 服务（iam-service / notification-service / audit-service / file-service / token-service / prue_backend）
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
| file-service         | 8086³              | 8082             | MySQL / Consul                            | ✅ |
| consul               | 8500               | —                | —                                         | — |
| redis                | 6379               | —                | —                                         | — |
| mysql                | 3306               | —                | root / rootpass                           | — |

> ¹ **token-service / audit-service / prue_backend 的 `cargo run` 默认端口**与 compose 不一致——code default 是 8080（audit / prue_backend）或 4000（token-service），compose 显式 `SERVER_PORT=...` 覆盖。开发时直跑 cargo 不带 env 会撞 8080 / 4000；带 compose env 后才落到 808x。
>
> ² **iam-service compose host 端口 9081** 是 Phase 5-A mesh 时代 sidecar 外端口残留（容器内一直是 8081），spec §7.3 规划统一到 `8081:8081`，待落地。
>
> ³ **file-service 的 host 端口 8086** 是 Phase 5-A-2 引入的消歧（容器内与 token-service 同端口，靠 host 端口区分）。
>
> 跨服务调用走 **容器内端口**（`http://iam-service:8081` 等），由 Docker `superpowers` 网络 DNS 解析；host 端口只用于宿主机 `curl` 开发联调，gateway 不走 host 端口。详见 [`docs-site/user-manual/11-reference.md` §11.5](docs-site/user-manual/11-reference.md#115-服务端口速查phase-5-a-2-之后) 与 [`docs-site/user-manual/02-architecture.md` §2.5](docs-site/user-manual/02-architecture.md#25-swagger-ui-覆盖)。
>
> ✅ = 启用 OpenTelemetry traces（跨服务 trace 链在 Jaeger 可视化）

### 可观测性

详见 [`docs-site/guides/observability.md`](docs-site/guides/observability.md)。

```bash
docker compose -f infrastructure/observability/docker-compose-observability.yml up -d
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
| file-service         | 8086               | http://localhost:8086/swagger-ui  | http://localhost:8086/api-docs/openapi.json |

> 全部 6 个业务服务的 openapi.yaml 都是 utoipa 自动生成的。`api-gateway` 是代理，无自有路由（详情见 `services/api-gateway/openapi.yaml`）。详见 `.agents/skills/coder-openapi-contracts-sync`。

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
- `observability/docker-compose-observability.yml` — overlay（Prometheus + Grafana + Jaeger + OTel collector）

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
- **OpenAPI drift CI 失败**：按 `.agents/skills/coder-openapi-contracts-sync` 重新生成 spec 并 commit

## Services

| Service              | Language | Framework | OTel |
| -------------------- | -------- | --------- | ---- |
| API Gateway          | Rust     | Axum      | ✅ |
| Token Service        | Rust     | Axum      | ✅ |
| IAM Service          | Rust     | Axum      | ✅ |
| Notification Service | Rust     | Axum      | ✅ |
| Audit Service        | Rust     | Axum      | ✅ |
| File Service         | Rust     | Axum      | ✅ |
| Prue Backend         | Rust     | Axum      | ✅ |

> 旧 `auth-service` / `user-service` 已废弃并合并进 `iam-service`。

## AI 辅助开发

项目使用 `.agents/skills/` 提供 AI skill：

| Skill | 用途 |
|-------|------|
| `coder-add-route` | 给现有 service 加路由（含 utoipa 注解 + openapi.rs 注册） |
| `coder-migrate-db` | 加迁移（MySQL + SQLite 双版本） |
| `coder-new-service` | 脚手架新 Rust microservice（满足 8-item 服务规范） |
| `coder-sdk-publish-rust` | 发布 Rust crates 到 kellnr 私有 registry（libs/* 17 + sdk/rust/* 6，含拓扑顺序与 dry-run） |
| `coder-sdk-publish-maven` | 发布 Java SDK 到 Reposilite（release + snapshot，含 `mvn versions:set` 自动 bump + curl 上传模式 + 4 级凭证 fallback） |

> 发布也可以直接用 CLI：`coder release images|sdk|status`（见「[部署与发布](#部署与发布)」），
> 版本真相源是根 `Cargo.toml` 的 `[workspace.package].version`。

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

分两档：**单机全栈**用示例单元（推荐），**三机组**走
[`docs/deploy-topologies.md`](docs/deploy-topologies.md) 的模式 C。

```bash
# ── 单机全栈（示例单元，整目录分发即可）──────────────────────────
scp -r examples/docker-deploy-full deploy-host:~/lmms/
ssh deploy-host
cd ~/lmms/docker-deploy-full
cp .env.example .env                       # 填中间件口令 / bootstrap token
./start.sh --no-build                      # 拉 registry 镜像（不本地构建）
./start.sh --logs                          # 需要日志栈时叠加 loki+promtail

# 升级到新版本：镜像 tag 已在 .image.env 里钉版
docker compose -p superpowers-full pull
./start.sh --no-build

# ── 仅业务服务（中间件由他机/他组提供）──────────────────────────────
cd examples/docker-deploy
cp .env.example .env
./start.sh --no-build
```

> 镜像版本不手改 compose：`examples/*/.image.env` 是唯一真相源（发布时由
> `coder release images --push` 写入）。换版本 = 更新 `.image.env` 后
> `docker compose pull`。

## 部署与发布

### 两个示例部署单元

`examples/` 下有两个**自包含部署单元**——整目录 copy 到目标机、`./start.sh` 即起：

| 目录 | 组成 | project 名 | 用途 |
|------|------|-----------|------|
| [`examples/docker-deploy/`](examples/docker-deploy/) | services（7 服务 + 2 outbox） | `superpowers-svc` | 中间件由他机/他组提供，只跑业务 |
| [`examples/docker-deploy-full/`](examples/docker-deploy-full/) | **infra（4）+ observability（6，`--logs` 再 +2）+ services** | `superpowers-full` | 单机全栈 |

```bash
# 全栈单机
cd examples/docker-deploy-full
cp .env.example .env        # 填中间件口令与 bootstrap token
./start.sh                  # 业务端口只留 gateway:3000
./start.sh --logs           # 叠加 loki + promtail
./stop.sh --volumes         # 连数据卷一起清

# 仅业务服务
cd examples/docker-deploy
cp .env.example .env
./start.sh
```

入口：gateway `:3000`｜Grafana `:3001`｜Prometheus `:9090`｜Jaeger `:16686`｜Consul `:8500`

### 镜像引用（`.image.env`，发布即固化）

compose 的 `image:` **只写整名变量**（`${IMAGE_FILE_SERVICE}`），值集中在各单元的
`.image.env`（入库，无凭据）。用 `:?` 必填语法——缺变量时 compose 直接报错，
不会静默展开成非法镜像名。`start.sh` 加载顺序：`.env` → `.env.prod` →
**`.image.env`（最后，权威）**，并把它们以 `--env-file` 透传给 compose。

`.image.env` 分两段：

| 段 | 内容 | 维护 |
|----|------|------|
| 平台镜像 | `IMAGE_API_GATEWAY` / `IMAGE_TOKEN_SERVICE` …（7） | **机器生成**，版本 = 服务 crate 版本 + 方言后缀（`-pg`）；两单元逐字一致 |
| 基础镜像 | `IMAGE_CONSUL` / `IMAGE_POSTGRES` / `IMAGE_PROMETHEUS` …（12，仅 full） | **手工维护**：首次写默认值，之后改哪行保留哪行（升 postgres 主版本、换源改这里） |

### 发布流程

版本号**唯一真相源**是根 `Cargo.toml` 的 `[workspace.package].version`（8 个
`services/*` 用 `version.workspace = true` 继承）。`release.toml` 只放方言 /
镜像源 / SDK 目标。

```bash
# 1) 改版本：编辑根 Cargo.toml 的 [workspace.package].version（如 1.7.0）
#    （任一服务写死别的版本 → release images 报 VERSION_SPLIT 并按错）

# 2) 体检：服务版本 / 两个 .image.env 钉版 / git tag 是否一致
node packages/cli/bin/coder.js release status

# 3) 构建并推送镜像；成功后自动写两个单元的 .image.env
node packages/cli/bin/coder.js release images --push
#    pg 方言跑两批：先 api-gateway（无后缀 tag），再 6 个 DB 服务（-pg 后缀）

# 4) 提交 .image.env 的 diff，打 git tag v<version>
#    （tag 带 v、镜像 tag 不带 v；CI 用 ${CI_COMMIT_TAG#v} 剥 v）

# 5) 发布 SDK（各生态版本独立：libs/*+sdk/rust → kellnr；npm → 私有 registry；sdk/java → Maven）
node packages/cli/bin/coder.js release sdk all --dry-run
node packages/cli/bin/coder.js release sdk all
```

> 底层脚本：`node scripts/update-image-env.js`（写 / `--check` 门禁 / `--verify`
> 探测 registry）。CI `deploy_image_env_check` job 跑 `--check`：它比对的是
> **服务 crate 版本**，不是文件自身——所以「改了 Cargo 版本忘更新钉版」会被拦下。

详见 [`docs/deploy-topologies.md`](docs/deploy-topologies.md)「镜像引用与版本钉版」。

## Infrastructure

- **Consul**: Service discovery（服务进程自注册，SIGTERM 注销）
- **Redis**: Token storage, caching
- **RabbitMQ**: 跨服务事件（transactional outbox）
- **PostgreSQL / MySQL**: 持久化存储（双方言，构建期 `SERVICE_FEATURES` 选择）

## Configuration

配置分两处：

- **部署单元自包含**（`examples/docker-deploy*/config/`）：
  - `api-gateway/api-gateway.toml` — 网关全局设置（server / redis / jwks / consul）
  - `api-gateway/routes/*.router.toml` — 路由规则，一个 URL 前缀一个文件
  - `docker-deploy-full` 独有：`config/infra/`（consul connect / rabbitmq）、
    `config/observability/`（prometheus / grafana / otel / alertmanager / loki / nginx）
- **服务自身**：`services/<svc>/config.default.yaml`（首次启动 seed，
  `CONFIG_PATH` 可覆盖；dev 环境 compose bind-mount 到 `/etc/<svc>/`）

API Gateway routing is split per URL prefix; each `routes/<name>.router.toml` declares its `match` pattern, target `service`, `public` flag and optional `rewrite` rule chain. The gateway exposes a `validate` subcommand for CI checks and a `serve` subcommand with `SIGHUP` / `POST /__admin/reload` for hot reload. See `docs-site/guides/api-gateway-routing.md` for the full usage manual and `docs-site/decisions/ADR-001-api-gateway-multi-file-router-config.md` for rationale.

## NPM Scripts

项目使用 pnpm workspace 管理，通过根目录的 `package.json` 统一调度。

### 前置依赖

```bash
# 安装 pnpm
npm install -g pnpm

# 安装工作区依赖
pnpm install

# Debian/Ubuntu: 准备 NASM (ring 0.17.x 编译时需要)；首次还需下载 vendored swagger-ui
node packages/cli/bin/coder.js assets download
pnpm run check:env
```

### 根目录脚本

| 脚本                                         | 作用                                              |
| -------------------------------------------- | ------------------------------------------------- |
| `pnpm run build`                             | 编译 api-gateway 与 token-service（Rust release） |
| `pnpm run dev:pg` / `dev:mysql`              | 启动对应方言的基础设施，并并行运行各服务 dev 任务 |
| `pnpm run dev:infra:pg` / `dev:infra:mysql`  | 仅启动 Docker 基础设施（consul / redis / rabbitmq / postgres 或 mysql） |
| `pnpm run check:gates`                       | 迁移/命名门禁（`check-db-naming` + `check-pg-migrations`） |
| `pnpm run test`                              | 在所有 workspace 包中执行 `test`                  |
| `pnpm run clean`                             | 在所有 workspace 包中执行 `clean`                 |
| `pnpm run docker:build`                      | 构建服务镜像（`coder build images`；支持 `-v`/`--push`/`--dialect`） |
| `pnpm run release:status`                    | 版本体检：服务版本 / `.image.env` 钉版 / git tag |
| `pnpm run release:images -- --push`          | 按服务 crate 版本构建+推送，成功后写两个单元的 `.image.env` |
| `pnpm run release:sdk -- all`                | 发布 SDK（`rust` / `npm` / `java` / `all`，各生态版本独立） |
| `pnpm run deploy:image-env:check`            | 校验 `.image.env` 与服务 crate 版本一致（CI 同款门禁） |
| `pnpm run check:env`                         | 检查开发环境服务状态                              |
| `pnpm run ai:review`                         | AI 代码审查，自动识别不符合规范的代码             |
| `pnpm run ai:gen-e2e`                        | 从 OpenAPI 文档自动生成 E2E 测试用例             |
| `pnpm run contracts:gen`                     | 生成契约代码（错误码、权限码）                    |
| `pnpm run check:service-standard`            | 检查服务规范合规性                                |
| `pnpm run check:spec`                        | 检查契约规范（权限码、错误码、OpenAPI）           |
| `cargo run -p <svc> --bin dump_openapi` 或 skill `coder-openapi-contracts-sync` | 重新生成 service 的 openapi.yaml |

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

# packages/（前端 TS 集成包：@digua/contracts + @digua/http）+ sdk/*-services（npm TS SDK）
pnpm run packages:build      # 构建全部包
pnpm run release:sdk -- npm  # 发布到私有 registry（含构建；版本在各自 package.json）
```

### 常用工作流

```bash
# 本地开发（pg 方言；mysql 用 dev:mysql / dev:infra:mysql）
pnpm run dev:infra:pg
pnpm run dev:pg

# 单独构建某个服务
pnpm --filter token-service run build

# 完整 CI 流程
pnpm run check:gates && pnpm run test && pnpm run build

# 发布：先改根 Cargo.toml 的 [workspace.package].version
pnpm run release:status                    # 体检
pnpm run release:images -- --push          # 构建 + 推送 + 更新 .image.env
pnpm run release:sdk -- all                # 发布 SDK

# 只构建镜像（不发布，不更新钉版）
pnpm run docker:build -- -v 1.0.0 --dialect pg --push
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
| **部署单元与镜像钉版** | [`docs/deploy-topologies.md`](docs/deploy-topologies.md) |
| **全栈示例部署** | [`examples/docker-deploy-full/`](examples/docker-deploy-full/) |
| **仅业务服务示例部署** | [`examples/docker-deploy/`](examples/docker-deploy/) |
