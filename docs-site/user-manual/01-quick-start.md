# 01. 快速上手

> **目标**：15 分钟内把基础设施 + 至少一个服务跑起来。
> 新人 / 新 AI 实例**从这里开始**。

---

## 1.1 前置依赖

| 工具 | 版本 | 用途 |
|------|------|------|
| Docker Desktop | 24+ | 跑 MySQL / Redis / Consul / 可观测性栈 |
| Rust toolchain | 1.96+（`rustup`） | 编译所有 Rust 服务 |
| Node.js | 18+ | 跑 CI 门禁脚本与 E2E 驱动 |
| pnpm | 8+ | 管理 E2E workspace |

> **关于本地 `cargo` 编译**：本机可能缺 `cc` 链接器或 musl target。Rust 测试统一走容器 `node scripts/run-tests-docker.js <svc>`，避免折腾本地 toolchain。

---

## 1.2 首次环境准备

```bash
# 1. 克隆并安装 workspace 依赖
git clone <repo-url> coder-micro-services
cd coder-micro-services
pnpm install

# 2. 配置 Docker 镜像来源 + 同步镜像清单 + 拉镜像 + 下载 swagger-ui + 环境体检
pnpm run setup
# 其中的 setup-wizard 会交互式询问两类镜像来源：
#   - 基础镜像（redis/mysql/consul 等）：留空=Docker Hub，或输入内网 mirror
#   - Builder 镜像（rust/java 等）：默认 zhcoder-docker-registry.com:8000
# 详见 ../guides/docker-registry-config.md

# 3. 启动基础设施（MySQL / Redis / Consul / 业务服务 / 可观测性）
docker compose -f infrastructure/docker-compose.yml up -d

# 4. 校验基础设施健康
docker ps --filter "network=superpowers"
# 应看到：8 个业务服务 + consul + redis + mysql 全部 UP
```

> **内网环境**：若 Docker Hub 不可达，`pnpm run setup` 时基础镜像填 `zhcoder-docker-registry.com:8000/official`。CI 用 `CODER_BASE_REGISTRY` 环境变量覆盖。

---

## 1.3 跑一个服务

**两种方式**：

### A. 本地 `cargo run`（最常用，调试友好）

```bash
cd services/iam-service

# 跑 cargo 测试（用 docker，无需本地 cc 链接器）
node scripts/run-tests-docker.js iam-service

# 启动服务
cargo run --bin iam-service
# 监听 0.0.0.0:8081（`SERVER_PORT` 默认值；compose 下走 9081:8081 host 端口）
# Swagger UI：http://localhost:8081/swagger-ui（需 SWAGGER_ENABLED=true）
```

### B. docker compose 全栈跑（接近生产）

```bash
docker compose -f infrastructure/docker-compose-infra.yml up -d
docker compose -f infrastructure/docker-compose-services.yml up -d

# 直连 HTTP（Phase 5-A-2 起无 mesh / 无 sidecar / 无 mTLS）：
# 浏览器走 api-gateway：http://localhost:3000/v1/iam/health
# Swagger UI 直连各服务，端口见 [02 §2.5](./02-architecture.md#25-swagger-ui-覆盖)
# 完整拓扑与跨服务调用机制见 [02 §2.1](./02-architecture.md)
```

---

## 1.4 跑端到端测试

```bash
# 冒烟子集 — 2 分钟
cd tests/e2e
pnpm install
pnpm test:smoke

# 全量 API 测试 — 约 5 分钟
pnpm test:api
```

> E2E 只有数据驱动 API 测试，直接打 api-gateway:3000（原 Playwright UI 套件已随 PRISM console 移除删除）。

---

## 1.5 下一步

| 想做什么 | 跳到 |
|---------|------|
| 理解服务怎么串起来 | [02 架构总览](./02-architecture.md) |
| 本地开发日常工作流 | [03 本地开发](./03-local-development.md) |
| 写测试 | [04 测试](./04-testing.md) |
| 看 AI Agent 怎么协作 | [09 AI 协作工作流](./09-ai-workflows.md) |
| 跑挂了怎么查 | [10 故障排查](./10-troubleshooting.md) |
