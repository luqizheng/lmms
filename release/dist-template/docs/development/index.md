# 业务开发指南

本文档面向**在 coder-micro-services 平台上开发业务微服务的开发人员**。
你手头的分发包已包含 8 个平台核心服务的 Docker 镜像和一个空的 `services/` 目录——
本指南教你如何在这里创建、集成、测试和部署自己的业务服务。

---

## 1. 平台架构速览

在你开始写代码之前，先理解平台已经替你做了哪些事情。

```
                      ┌──────────────────────────────┐
                      │      nginx / CDN / LB        │
                      └─────────────┬────────────────┘
                                    │
                      ┌─────────────▼────────────────┐
                      │      api-gateway :8080        │
                      │   JWT 验签 · 限流 · 路由分发  │
                      └─────────────┬────────────────┘
                                    │
          ┌────────────┬────────────┼────────────┬────────────┐
          │            │            │            │            │
     ┌────▼───┐  ┌────▼───┐  ┌────▼───┐  ┌────▼───┐  ┌────▼───┐
     │ token  │  │  iam   │  │ audit  │  │ config │  │ 你的    │
     │ :8082  │  │ :8084  │  │ :8086  │  │ :8081  │  │业务服务 │
     └────┬───┘  └────┬───┘  └────┬───┘  └────┬───┘  └────┬───┘
          │            │            │            │            │
          └────────────┴────────────┴────────────┴────────────┘
                                    │
          ┌─────────────────────────┼─────────────────────────┐
          │  MySQL :3306  │  Redis :6379  │  RabbitMQ :5672   │
          │  Consul :8500  (service discovery)                │
          └───────────────────────────────────────────────────┘
                                    │
          ┌─────────────────────────▼─────────────────────────┐
          │  Prometheus :9090  │  Grafana :3001  │  Jaeger    │
          └───────────────────────────────────────────────────┘
```

### 平台已经提供

| 能力 | 说明 | 你不需要做什么 |
|------|------|-------------|
| **API 网关** | JWT 验签、限流、路由分发、canary | 不用自建网关，配路由即可 |
| **认证鉴权** | IAM 服务负责用户/角色/权限 | 不用自建用户系统 |
| **审计日志** | audit-service 自动记录操作 | 调 SDK 即可，审计自动落库 |
| **配置中心** | config-service 键值存储 | 不用自建配置管理 |
| **文件存储** | file-service 上传/下载 | 不用对接 S3/MinIO |
| **服务发现** | Consul 自动注册 + DNS 解析 | 不用手动管理 IP |
| **可观测性** | Prometheus + Grafana + Jaeger | 业务服务自动接入 OTel |
| **事件总线** | RabbitMQ + transactional outbox | 可靠异步消息开箱即用 |
| **多租户** | tenant_id 列 + TenantScope extractor | 按平台规约加 tenant_id 即可 |

---

## 2. 目录结构

解压发布包后，你的工作区结构如下：

```
coder-micro-services/
├── infrastructure/           # Docker Compose（唯一部署真相源）
│   ├── docker-compose.yml             # 主入口（include 下面的 yml）
│   ├── docker-compose-infra.yml       # 基础设施
│   ├── docker-compose-services.yml    # 所有服务（包括你将新增的业务服务）
│   └── config/
│       └── api-gateway/routes/        # 网关路由规则
├── services/                 # ← 你的业务服务写在这里
│   └── .gitkeep
├── contracts/                # 契约层（权限码 / 错误码 / 事件 / 规约）
│   ├── error-codes.yaml
│   ├── permissions.yaml
│   ├── events.yaml
│   ├── db-conventions.md
│   └── http-conventions.md
├── scripts/                  # CI 门禁脚本
├── .agents/skills/           # AI 辅助开发的 6 个 skill
├── Cargo.toml                # Rust workspace 根
├── pnpm-workspace.yaml       # pnpm workspace
└── .env                      # 环境变量（make secrets 生成）
```

### 你需要关心的目录

| 目录 | 作用 |
|------|------|
| `services/` | 你的业务服务源码 |
| `infrastructure/docker-compose-services.yml` | 把你的服务加入编排 |
| `infrastructure/config/api-gateway/routes/` | 把你的服务路由加入网关 |
| `contracts/` | 声明权限码、错误码、事件 |

---

## 3. 开发环境搭建

### 3.1 前置依赖

```bash
# Rust 工具链（用于 Rust 业务服务）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add x86_64-unknown-linux-musl

# Java（用于 Java 业务服务，可选）
# JDK 21 + Maven 3.9+

# Node.js（跑 CI 门禁脚本）
# 18+

# Docker（构建镜像、运行平台）
# 24.0+
```

### 3.2 配置 Rust registry

`.cargo/config.toml` 已预配置：

- `kellnr` 私有 registry — 拉取 Rust SDK 和 `libs/*` 平台库
- `rsproxy` 镜像 — crates.io 国内加速

使用前需设置 kellnr 凭证：

```bash
export CARGO_REGISTRIES_KELLNR_TOKEN=<your-token>
```

> `libs/*`（17 个平台基础设施 crate）和 `sdk/rust/*`（7 个 Rust SDK crate）均通过 kellnr registry 发布，
> 不随分发包提供源码。你的业务服务通过 `cargo add --registry kellnr` 引入。

### 3.3 配置 Java Maven（可选）

```xml
<!-- ~/.m2/settings.xml -->
<settings>
  <servers>
    <server>
      <id>kudi-maven-releases</id>
      <username>your-username</username>
      <password>your-password</password>
    </server>
  </servers>
</settings>
```

### 3.4 启动平台（开发模式）

```bash
# 1. 初始化密钥
make secrets

# 2. 启动平台（你的业务服务尚未创建，平台核心服务 + 基础设施先行）
make up

# 3. 验证
make verify
```

---

## 4. 创建第一个业务服务

平台通过 `.agents/skills/` 下的 AI skill 提供代码生成能力。
你只需用自然语言描述需求，AI 会自动生成服务骨架、路由、迁移文件。

### 4.1 选择技术栈

| 场景 | 技术栈 | Skill |
|------|--------|-------|
| 高并发、低延迟、事件驱动 | Rust + Axum | `coder-new-service` |
| 企业级、Spring 生态、存量团队 | Java + Spring Boot 3 | `coder-new-java-service` |

### 4.2 Rust 业务服务

Skill `coder-new-service` 会生成：

```
services/my-order-service/
├── Cargo.toml
├── config/
│   └── config.default.yaml    # 运行时配置模板
├── migrations/
│   └── mysql/
│       └── V001__create_order_table.sql
├── src/
│   ├── main.rs
│   ├── routes/
│   │   └── mod.rs
│   ├── models.rs
│   └── openapi.rs
├── openapi.yaml               # utoipa 自动生成
└── README.md
```

生成会自动完成：
- **Cargo.toml member**：追加到 `Cargo.toml` workspace members
- **Docker Compose**：在 `infrastructure/docker-compose-services.yml` 追加 service 定义
- **网关路由**：在 `infrastructure/config/api-gateway/routes/` 追加路由规则
- **8-item 检查豁免**：（如需要）在 `services/AGENTS.md` 声明豁免项

### 4.3 Java 业务服务

Skill `coder-new-java-service` 生成标准 Spring Boot 3 多模块项目：

```
services/my-order-service/
├── pom.xml                    # 父 POM
├── my-order-service-server/   # 服务模块
│   ├── pom.xml
│   ├── src/main/java/...
│   └── src/main/resources/
│       ├── application.yml
│       └── db/migration/      # Flyway 迁移
├── my-order-service-api/      # API DTO 模块
│   ├── pom.xml
│   └── src/main/java/...
└── openapi.yaml
```

### 4.4 添加路由（已有服务）

在已有服务中添加新端点，使用：

- Rust：skill `coder-add-route`
- Java：skill `coder-add-java-route`

---

## 5. 把业务服务接入平台

创建服务后，需要完成以下集成步骤：

### 5.1 加入 Docker Compose

编辑 `infrastructure/docker-compose-services.yml`，在 `services:` 下追加：

```yaml
  my-order-service:
    image: ${REGISTRY:-zhcoder-docker-registry.com:8000}/micro-server/my-order-service:${IMAGE_TAG:-v1.0.0}
    container_name: coder-micro-services-my-order-service
    environment:
      <<: *svc-env
      MY_ORDER_DEV_BT: ${MY_ORDER_DEV_BT}
    volumes:
      - ./config/my-order:/app/config:ro
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
    networks:
      - coder-net
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 15s
      timeout: 5s
      retries: 5
      start_period: 30s
```

> 如果用 AI skill 创建，此项自动完成。

### 5.2 加入网关路由

在 `infrastructure/config/api-gateway/routes/` 下新建 `my-order.router.toml`：

```toml
[match]
prefix = "/v1/orders"

[service]
name = "my-order-service"
port = 8080

[rate_limit]
requests_per_second = 50
burst = 100

[[routes]]
path = "/v1/orders"
methods = ["GET", "POST"]
public = false       # 需要认证

[[routes]]
path = "/v1/orders/health"
methods = ["GET"]
public = true        # 健康检查无需认证
```

### 5.3 注册 bootstrap token

在 `.env` 中添加：

```env
MY_ORDER_DEV_BT=__CHANGE_ME__
```

然后重新运行 `make secrets`，或在 `SERVICE_BOOTSTRAP_TOKENS` 中追加。

### 5.4 声明契约

#### 权限码

在 `contracts/permissions.yaml` 中声明业务权限：

```yaml
# 新增业务权限组
order:
  permissions:
    - ORDER_CREATE
    - ORDER_READ
    - ORDER_UPDATE
    - ORDER_DELETE
```

然后运行：

```bash
node scripts/contracts-gen.js
```

这会重新生成 Rust 权限码常量（`libs/digua_authz/src/perms.rs`），你的服务可以通过 `digua_common-digua_authz` crate 引用。

#### 错误码

在 `contracts/error-codes.yaml` 中声明业务错误码：

```yaml
# 新增业务错误码
order-catalog:
  codes:
    - ORDER_NOT_FOUND: 1001
    - ORDER_STATUS_INVALID: 1002
    - INSUFFICIENT_INVENTORY: 1003
```

同样运行 `node scripts/contracts-gen.js` 后，通过 `digua_common-error` crate 引用。

#### 事件

在 `contracts/events.yaml` 中声明事件定义：

```yaml
# 新增业务事件
order:
  events:
    - ORDER_CREATED
    - ORDER_CANCELLED
    - ORDER_PAID
```

事件通过 RabbitMQ + transactional outbox 发送，保证事务一致性。

### 5.5 数据库与迁移

#### 表名前缀规约

所有表必须带服务前缀，由 CI gate 强制执行。例如：

```sql
-- ✅ 正确
CREATE TABLE order_orders (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  ...
);

-- ❌ 错误（缺少 order_ 前缀）
CREATE TABLE orders (...);
```

#### Rust 迁移

使用 skill `coder-migrate-db` 创建迁移：

```bash
# SQL 迁移文件放在 services/my-order-service/migrations/mysql/
# V001__create_order_table.sql
# V002__add_status_column.sql
```

Rust 服务使用 `sqlx` 的 `migrate!()` 宏自动执行迁移。

#### Java 迁移

使用 skill `coder-migrate-java-db`：

```
# Flyway 文件放在 services/my-order-service/my-order-service-server/src/main/resources/db/migration/
# V1__create_order_table.sql
# V2__add_status_column.sql
```

Spring Boot + Flyway 会在启动时自动执行。

---

## 6. 使用 SDK

### 6.1 Rust SDK

通过 kellnr registry 引入平台服务 SDK：

```bash
# 调用 IAM 服务
cargo add iam-service-sdk --registry kellnr

# 调用审计服务
cargo add audit-service-sdk --registry kellnr

# 调用配置中心
cargo add config-service-sdk --registry kellnr

# HTTP 客户端（JWT 自动注入、重试、熔断）
cargo add kudi-http-client --registry kellnr

# 平台基础设施库
cargo add digua_common-error --registry kellnr
cargo add digua_common-db --registry kellnr
cargo add digua_common-log --registry kellnr
```

或在 `Cargo.toml` 中手动添加：

```toml
[dependencies]
iam-service-sdk = { version = "0.1", registry = "kellnr" }
audit-service-sdk = { version = "0.1", registry = "kellnr" }
digua_common-error = { version = "0.1", registry = "kellnr" }
```

### 6.2 Java SDK

在业务服务的 `pom.xml` 中声明 Maven 仓库和依赖：

```xml
<repositories>
  <repository>
    <id>kudi-maven-releases</id>
    <url>http://192.168.2.12:18181/releases</url>
  </repository>
</repositories>

<dependencies>
  <dependency>
    <groupId>com.kudi.coder</groupId>
    <artifactId>iam-service-sdk</artifactId>
    <version>0.3.0</version>
  </dependency>
  <dependency>
    <groupId>com.kudi.coder</groupId>
    <artifactId>audit-service-sdk</artifactId>
    <version>0.3.0</version>
  </dependency>
</dependencies>
```

> 版本号以 `sdk/java/pom.xml` 当前版本为准。

---

## 7. 事件总线

平台使用 **RabbitMQ + transactional outbox** 模式，保证事件发送的原子性。

### Rust 中发布事件

```rust
use digua_common_event::MySqlOutboxRepo;

// 在同一个事务中：写入业务数据 + enqueue 事件
let mut tx = pool.begin().await?;
// ... 业务操作 ...
sqlx::query("INSERT INTO order_orders (...) VALUES (...)")
    .execute(&mut *tx).await?;

// 同事务入队 outbox 事件
let outbox = MySqlOutboxRepo::new(&mut tx);
outbox.enqueue("ORDER_CREATED", &order_payload).await?;

tx.commit().await?;
// outbox-publisher 会自动读取并投递到 RabbitMQ
```

### 消费事件

在 Rust 服务中消费 RabbitMQ 消息：

```rust
use digua_common_event::EventConsumer;

// 声明队列 + 绑定 exchange
// digua_common-event 库提供了 RabbitMQ 消费者抽象
```

### Java 中发布事件

```java
// 使用 digua_common-event Java SDK（待发布）
// 或直接通过 Spring AMQP
```

---

## 8. 多租户

平台支持多租户隔离，租户 ID 存储在 `X-Tenant-Id` header。

### 业务服务接入

**Rust**：使用 `TenantScope` extractor

```rust
use common_tenant::TenantScope;

async fn list_orders(
    tenant: TenantScope,  // 自动从 request 提取 tenant_id
) -> impl IntoResponse {
    // tenant.id() 获取当前租户
    let orders = sqlx::query_as::<_, Order>(
        "SELECT * FROM order_orders WHERE tenant_id = ?",
    )
    .bind(tenant.id())
    .fetch_all(&pool)
    .await?;
    // ...
}
```

**Java**：类似通过 filter/interceptor 从 request 提取

### 数据库表设计

所有多租户表必须包含 `tenant_id` 列：

```sql
CREATE TABLE order_orders (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  tenant_id BIGINT NOT NULL,          -- 必需
  order_no VARCHAR(64) NOT NULL,
  -- ...
  INDEX idx_tenant (tenant_id)
);
```

> CI gate `check-tenant-scope.js` 会检查代码中是否遗漏 tenant_id 过滤。

---

## 9. CI 门禁

每次提交前必须通过以下检查：

```bash
# 服务规范（8-item）
node scripts/check-service-standard.js

# 表名服务前缀
node scripts/check-db-naming.js

# OpenAPI / 错误码 / 权限码
node scripts/check-spec.js

# 契约生成代码一致性
node scripts/contracts-gen.js --check
```

### 8-item 服务规范

每个服务必须满足：

| # | 检查项 | 说明 |
|---|--------|------|
| 1 | AGENTS.md | 服务级 AI 协作约定 |
| 2 | Dockerfile | 多阶段构建 |
| 3 | openapi.yaml | utoipa 生成 |
| 4 | /health 端点 | HTTP 健康检查 |
| 5 | /metrics 端点 | Prometheus 指标 |
| 6 | config/ | 运行时配置目录 |
| 7 | migrations/ | 数据库迁移 |
| 8 | README.md | 服务说明文档 |

### 豁免

某些服务可能需要豁免特定检查（如纯代理网关不需要 migrations）。
在 `services/AGENTS.md` 中声明豁免项，由 CI 识别放行。

---

## 10. 测试

### Rust 单元测试

```bash
# 单个服务
cargo test -p my-order-service --lib

# 全 workspace
cargo test --workspace --lib
```

### 集成测试（含 testcontainers）

```bash
# 需要 Docker daemon
node scripts/run-tests-docker.js my-order-service
```

### E2E 测试

分发包已包含 E2E 测试框架。安装依赖并运行：

```bash
pnpm -C tests/e2e install
pnpm -C tests/e2e test:smoke   # 冒烟测试（8 API + 2 UI 用例）
pnpm -C tests/e2e test:api     # 全量 API 测试
pnpm -C tests/e2e test:ui      # UI 测试（需前端容器）
```

业务服务应追加自己的 E2E case 到 `tests/e2e/api/cases/`。

---

## 11. 与 AI 协作开发

分发包已内置 AI 辅助开发框架。你在 IDE（如 Cursor / VS Code + Copilot / Codex / Trae）中打开项目后，AI 可直接使用 `.agents/skills/` 下的 skill。

### 给 AI 的指令示例

> "用 skill coder-new-service 创建一个 Rust 订单服务，包含创建订单和查询订单列表两个接口，数据存 MySQL order_orders 表"

> "给 order-service 添加一个取消订单的端点，状态从 pending → cancelled"

> "创建 migration V003，给 order_orders 表加 payment_status 列"

### Skill 清单

| Skill | 用途 |
|-------|------|
| `coder-new-service` | 新建 Rust 业务服务 |
| `coder-new-java-service` | 新建 Java 业务服务 |
| `coder-add-route` | 给 Rust 服务添加路由 |
| `coder-add-java-route` | 给 Java 服务添加路由 |
| `coder-migrate-db` | 创建 Rust 服务迁移 |
| `coder-migrate-java-db` | 创建 Java 服务迁移 |
| `plan-eng-review` | 复杂变更计划审查 |
| `karpathy-guidelines` | 代码质量规范 |

---

## 12. 构建与部署

### 构建镜像

```bash
# 构建你的业务服务镜像
docker build -t my-order-service:latest -f services/my-order-service/Dockerfile .

# 或使用 pnpm 脚本（如果已注册）
docker compose -f infrastructure/docker-compose.yml build my-order-service
```

### 上线业务服务

```bash
# 1. 构建镜像并推送到 registry
docker tag my-order-service:latest ${REGISTRY}/micro-server/my-order-service:v1.0.0
docker push ${REGISTRY}/micro-server/my-order-service:v1.0.0

# 2. 在生产环境拉取并重启
ssh deploy-server
cd /opt/coder-micro-services
docker compose -f infrastructure/docker-compose.yml pull my-order-service
docker compose -f infrastructure/docker-compose.yml up -d my-order-service
```

---

## 附录：快速参考卡片

### Rust 业务服务速查

```bash
# 创建服务（用 AI skill）
# → services/my-service/

# 引入 SDK
cargo add iam-service-sdk --registry kellnr
cargo add digua_common-error --registry kellnr
cargo add digua_common-db --registry kellnr

# 本地运行平台
make up

# 编译检查
cargo check -p my-service

# 格式化
cargo fmt --check

# Lint
cargo clippy -- -D warnings

# 单元测试
cargo test -p my-service --lib

# 集成测试
node scripts/run-tests-docker.js my-service

# CI 门禁
node scripts/check-service-standard.js
node scripts/check-db-naming.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
```

### Java 业务服务速查

```bash
# 创建服务（用 AI skill）
# → services/my-service/

# 编译
cd services/my-service && mvn compile

# 测试
mvn test

# 打包
mvn package -DskipTests

# CI 门禁
node scripts/check-service-standard.js
node scripts/check-db-naming.js
```

### 常用命令

```bash
make up              # 启动全部
make down            # 停止
make status          # 状态
make logs svc=xxx    # 日志
make verify          # 冒烟测试
make reset-data      # 重置数据（谨慎！）
```
