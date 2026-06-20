# coder-micro-services AI 协作指南

## Skill 索引

| Skill | 触发关键词 | 命令 |
|---|---|---|
| `coder-run-tests` | 测试、test | node scripts/run-e2e-local.js |
| `coder-base-images-sync` | base images、镜像清单 | node scripts/base-images-sync.js |
| `coder-route-validate` | 校验路由、reload | cargo run -p api-gateway -- validate |
| `coder-spec-to-code` | spec-to-code、写测试用例 | node scripts/spec-to-code.js |
| `coder-sdk-publish` | 发 SDK、sdk:publish | node scripts/sdk-publish.js |
| `coder-microservice-dev` | 起服务、本地开发 | node scripts/dev-services.js |
| `coder-service-standard-check` | 检查服务规范、standard | node scripts/check-service-standard.js |
| `coder-e2e-test` | E2E测试、登录验证 | node scripts/run-e2e-local.js |
| `coder-lib-upgrade` | 升级通用库 | cargo upgrade -p <lib> |
| `coder-service-audit` | 服务审计、规范检查 | node scripts/check-service-standard.js --service <name> |

## 服务规范规则系统

### 概述

每个服务目录下 `AGENTS.md` 定义服务开发规范。

### 规范内容

- Dockerfile
- Swagger 配置（环境变量控制）
- 健康检查端点
- 数据库迁移文件
- README.md
- tests/ 目录

### AI 检查时机

修改 `services/<name>/` 下文件时自动检查规范。

### 环境变量配置

- `SWAGGER_ENABLED`: 是否启用 Swagger
- `HEALTH_CHECK_REQUIRED`: 是否要求健康检查
- `MIGRATIONS_REQUIRED`: 是否要求迁移文件

## 单元测试

> 完整测试指南（分层、增删改、执行、AI 修测试、数据驱动 E2E）见 [docs/guides/testing.md](docs/guides/testing.md)。

### 执行方式（volume mapping 模型）

在 Docker 容器内跑 Rust 单元测试，不构建镜像，直接挂载源码与 cargo 缓存。

```bash
# 全部服务
node scripts/run-tests-docker.js

# 指定一个或多个服务
node scripts/run-tests-docker.js iam-service token-service
```

脚本 `scripts/run-tests-docker.js` 对每个服务执行：

```bash
docker run --rm --network host \
  -v ${ROOT}:/app \
  -v coder-cargo-registry:/usr/local/cargo/registry \
  -v coder-cargo-git:/usr/local/cargo/git \
  -v coder-cargo-target:/app/target \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -w /app \
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl \
  cargo test --target x86_64-unknown-linux-musl -p <service> -- --nocapture
```

### 规则

- 统一基础镜像 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`，target `x86_64-unknown-linux-musl`
- 源码 bind mount，改代码零 rebuild；`registry`/`git`/`target` 用命名 volume 跨 service 复用增量编译
- `--network host` 供 build.rs 联网（如 `utoipa-swagger-ui` 下载 swagger-ui）
- testcontainers 服务必须挂 `/var/run/docker.sock`
- `-p <service>` 限定范围，不跑 libs/sdk；多服务共享 target volume，串行执行避免锁争用
- TLS 依赖统一 workspace `reqwest 0.12 + rustls`，禁用 native-tls（musl 镜像无 openssl headers）
- 已废弃 `Dockerfile.test`，不再用 `docker build` 构建测试镜像

## 契约层

### 概述

`contracts/` 目录是前后端一致的单一真相源。

### 文件结构

| 文件 | 用途 |
|---|---|
| error-codes.yaml | 全局错误码字典 |
| permissions.yaml | 权限码字典 |
| events.yaml | 领域事件名（预留） |
| http-conventions.md | HTTP 响应/分页/排序约定 |

### 代码生成

```bash
node scripts/contracts-gen.js
```

生成目标：
- `libs/common-error/src/codes.rs` — Rust 错误码常量
- `libs/authz/src/perms.rs` — Rust 权限码常量
- `frontend/src/contracts/*.ts` — TypeScript 类型（预留）

### CI 校验

```bash
node scripts/contracts-gen.js   # 重新生成后用 git diff 检查是否有未提交改动
```

若生成结果与已提交代码不一致（git diff 非空），说明契约漂移，需重新提交生成产物。