# coder-micro-services v1.0.0 - 离线分发包

> **v2 架构**：本包采用「平台镜像 + 开发骨架」模式。8 个核心服务以 Docker 镜像发布（已推送仓库），`services/` 目录为空，用户通过 `skills/` 下的 6 个 skill 自助创建 Rust 或 Java 业务微服务。

## 目录结构

```
coder-micro-services/
├── bin/                    # 一键运维脚本
├── infrastructure/         # Docker Compose + 平台运行时配置（唯一真相源）
│   ├── docker-compose.yml           # 主入口（include 三个 yml）
│   ├── docker-compose-infra.yml     # 基础设施（MySQL/Redis/Consul/RabbitMQ/Registrator）
│   ├── docker-compose-services.yml  # 8 个核心服务（image: 引用仓库镜像）+ frontend
│   ├── docker-compose-observability.yml  # 可观测性（Prometheus/Grafana/Jaeger/OTel）
│   ├── config/             # 运行时配置（api-gateway routes / grafana / consul / ...）
│   │   ├── iam/            # iam-service 首启 seed 目标目录（含 config.default.yaml 模板）
│   │   └── notification/   # notification-service seed 目标（同上）
│   └── mysql-init/         # MySQL 初始化 SQL
├── frontend/               # 前端源码（pnpm-lock.yaml，不含 node_modules）
├── images/                 # 离线 Docker 镜像包 + manifest.json
├── secrets/                # 运行时生成的密钥（不提交 git）
│
├── ── v2 开发骨架 ──
│
├── services/               # 空 - 用户在此创建业务微服务
├── skills/                 # 6 个 AI skill（3 Rust + 3 Java）
├── .cargo/                 # Cargo registry 配置（kellnr 私有 registry + rsproxy 镜像）
├── contracts/              # 契约层（error-codes / permissions / events / conventions）
├── scripts/                # CI 门禁脚本（check-service-standard / check-db-naming / ...）
├── Cargo.toml              # Rust workspace 根（members: libs；sdk/rust/* 和 services/* 均被 sed 过滤）
├── pnpm-workspace.yaml     # pnpm workspace 配置
├── package.json            # 根调度脚本（CI 门禁 / 构建 / 测试命令）
├── AGENTS.md               # 项目协作指南（skill 引用的约定）
│
├── Makefile                # make 命令统一入口
└── .env.example            # 环境变量模板
```

## 快速开始

### 1. 启动平台（核心服务 + 基础设施 + 前端）

```bash
# 在线环境
make secrets && make up && make verify

# 离线环境
node bin/load-images.js && make secrets && make up && make verify
```

### 2. 创建业务微服务

获取发布包后，`services/` 目录为空。通过 skills 创建业务微服务：

**Rust 业务服务**：
```
# skill 生成服务骨架 + 自动追加 Cargo.toml member / docker-compose / 网关路由
# 详见 skills/coder-new-service/SKILL.md
```

**Java 业务服务**（Spring Boot 3 + JDK 21）：
```
# skill 生成多模块 Maven 骨架 + 自动追加 docker-compose / 网关路由 / 表前缀白名单
# 详见 skills/coder-new-java-service/SKILL.md
```

**加路由 / 迁移**：
- Rust：`skills/coder-add-route/` + `skills/coder-migrate-db/`
- Java：`skills/coder-add-java-route/` + `skills/coder-migrate-java-db/`

### 3. CI 门禁（创建业务服务后必须通过）

```bash
node scripts/check-service-standard.js   # 8-item 服务规范
node scripts/check-db-naming.js          # 表名服务前缀
node scripts/check-spec.js               # OpenAPI / 错误码 / 权限码
node scripts/contracts-gen.js --check    # 契约生成代码一致性
```

## 访问地址

| 服务 | 地址 | 默认账号 |
|------|------|---------|
| 前端 | http://localhost:3000 | - |
| API 网关 | http://localhost:8080 | - |
| Consul | http://localhost:8500 | - |
| Prometheus | http://localhost:9090 | - |
| Grafana | http://localhost:3001 | admin / admin |
| Jaeger | http://localhost:16686 | - |
| RabbitMQ | http://localhost:15672 | guest / guest |

## 核心服务清单（8 微服务 + 前端，镜像发布）

| 服务 | 端口 | 说明 |
|------|------|------|
| api-gateway | 8080 | API 网关（JWT 验签 + 限流 + 路由）|
| config-service | 8081 | 配置中心 |
| file-service | 8082 | 文件管理 |
| token-service | 8083 | Token 管理（outbox pattern）|
| iam-service | 8084 | 身份与访问管理 |
| notification-service | 8085 | 通知服务（outbox pattern）|
| audit-service | 8086 | 审计日志 |
| prue-backend | 8087 | 流程引擎（MySQL+SQLite）|
| frontend | 3000 | Vue 3 前端（nginx）|

## 业务服务开发流程

```
1. make up                                    # 启动平台
2. AI skill 生成业务服务骨架                      # services/{name}/
3. AI skill 自动追加 compose + 网关路由           # infrastructure/docker-compose-services.yml + infrastructure/config/api-gateway/routes/
4. 业务服务接入 docker compose up                # docker compose -f infrastructure/docker-compose.yml up -d {name}
5. CI 门禁通过                                   # check-service-standard / check-db-naming / check-spec
```

## SDK 引入（包管理器）

Rust SDK 和 Java SDK 均以包管理器发布，不打包进分发包源码。

### Rust SDK（私有 kellnr registry）

Rust SDK 各子 crate 和 libs/ 平台基础设施库均发布到私有 kellnr registry（`http://192.168.2.12:16444`）。业务 Rust 服务通过 `cargo add --registry kellnr` 引入：

```bash
# 按需添加对应服务的 SDK（--registry kellnr 指定从私有 registry 拉取）
cargo add audit-service-sdk --registry kellnr
cargo add config-service-sdk --registry kellnr
cargo add file-service-sdk --registry kellnr
cargo add iam-service-sdk --registry kellnr
cargo add digua-notification-sdk --registry kellnr
cargo add token-service-sdk --registry kellnr
cargo add digua-http-client --registry kellnr

# 平台基础设施库（业务 Rust 服务作为 axum web 时的地基）
cargo add digua_common-error --registry kellnr
cargo add digua_common-db --registry kellnr
cargo add digua_common-log --registry kellnr
```

或在 `Cargo.toml` 手动添加：

```toml
[dependencies]
audit-service-sdk = { version = "0.1", registry = "kellnr" }
iam-service-sdk = { version = "0.1", registry = "kellnr" }
digua_common-error = { version = "0.1", registry = "kellnr" }
digua_common-db = { version = "0.1", registry = "kellnr" }
```

cargo 会自动从已配置的 kellnr registry（`.cargo/config.toml`）拉取。编译前需设置 `CARGO_REGISTRIES_KELLNR_TOKEN` 环境变量。

### Java SDK（私有 Maven 仓库）

Java SDK 由 CI 在 `v*.*.*` tag 触发时推送到私有 Maven 仓库。业务 Java 服务通过 `<dependency>` 直接引入即可。

- **仓库地址**：`http://192.168.2.12:18181/releases`
- **groupId**：`io.digua`
- **version**：与 `sdk/java/pom.xml` 一致（当前 `0.3.0`）

### 1. 配置 Maven 仓库

在业务服务的 `pom.xml`（或 `~/.m2/settings.xml`）中声明仓库：

```xml
<repositories>
  <repository>
    <id>digua-maven-releases</id>
    <url>http://192.168.2.12:18181/releases</url>
    <releases><enabled>true</enabled></releases>
    <snapshots><enabled>false</enabled></snapshots>
  </repository>
</repositories>
```

如仓库需鉴权，在 `~/.m2/settings.xml` 的 `<servers>` 中补上：

```xml
<servers>
  <server>
    <id>digua-maven-releases</id>
    <username>你的用户名</username>
    <password>你的密码</password>
  </server>
</servers>
```

### 2. 引入 SDK 依赖

按需引入对应服务的 SDK（所有模块共享 `io.digua:digua-sdk-common`，会自动传递依赖）：

| SDK 模块 | artifactId | 用途 |
| --- | --- | --- |
| digua_common | `digua-sdk-common` | 公共配置/异常（其余 SDK 自动传递依赖） |
| digua-http-client | `digua-http-client` | 底层 HTTP 客户端封装 |
| digua-user-user-sdk | `digua-user-user-sdk` | 用户身份视角调用 IAM |
| digua-user-client-sdk | `digua-user-client-sdk` | 客户端应用视角调用 IAM |
| digua-token-user-sdk | `digua-token-user-sdk` | 用户身份视角调用 Token |
| digua-token-client-sdk | `digua-token-client-sdk` | 客户端应用视角调用 Token |
| audit-service-sdk | `audit-service-sdk` | 调用审计服务 |
| config-service-sdk | `config-service-sdk` | 调用配置中心 |
| file-service-sdk | `file-service-sdk` | 调用文件服务 |
| digua-notification-sdk | `digua-notification-sdk` | 调用通知服务 |

示例（以 audit-service-sdk 为例）：

```xml
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>audit-service-sdk</artifactId>
  <version>0.3.0</version>
</dependency>
```

> 版本号请对齐实际发布的 tag（`v0.3.0` -> `0.3.0`）。可通过 `sdk/java/pom.xml` 的 `<version>` 字段或 Maven 仓库 UI 确认。

## 镜像版本

所有核心服务镜像统一版本：`${IMAGE_TAG}`
Registry：`${REGISTRY}`

## 离线镜像打包（打离线下发包）

在**开发仓库根目录**、有网环境执行（建议 16GB+ 磁盘）。注意：**传入的是 no-v 版本**（git tag 是 `v1.0.0`，但镜像 tag / 产物名都不带 `v`）：

```bash
node release/dist-template/scripts/release.js 1.0.0     # 早期 shell 版 release.sh 已迁 Node 删除
pnpm run release:build                                  # 等价：默认版本（no-v）

# 输出：
#   release/coder-micro-services-1.0.0-airgap.tar.gz
#   release/coder-micro-services-1.0.0/
```

### 选项

| 用法 | 行为 |
| --- | --- |
| `node release/dist-template/scripts/release.js [VERSION]` | 默认：pull + save 全部镜像 + 同步开发骨架 |
| `node release/dist-template/scripts/release.js v1.2.0 --no-images` | 跳过 docker pull/save，仅打文档/配置/骨架包 |
| `EXPORT_IMAGES=0 node release/dist-template/scripts/release.js [VERSION]` | 等价 `--no-images`（环境变量风格） |
| `node release/dist-template/scripts/release.js --images [VERSION]` | 显式导出镜像（与默认等价，便于 CI 声明） |
| `node release/dist-template/scripts/release.js --help` | 打印完整用法 |

- `--no-images` 同时跳过 pull 和 save；离线安装时需先 `docker pull` 或重跑 `EXPORT_IMAGES=1 node release/dist-template/scripts/release.js`。
- flag 优先级高于 `EXPORT_IMAGES` 环境变量；`REGISTRY` 环境变量可覆盖默认仓库。
- v2 新增：release.js 自动从开发仓库同步 `.agents/skills/` + `.agents/rules/` / `scripts/` / `contracts/` / `frontend/`（不含 `node_modules` / `dist` / 缓存）+ `tests/e2e`（清理产物）+ `services/`（空骨架 + agents-template.md）+ `Cargo.toml` 到发布包。**`libs/` 和 `sdk/` 均不打包进分发包**：libs/*（17 个平台基础设施 crate）和 sdk/rust/*（7 个 Rust SDK crate）由 CI rust_publish job 发布到私有 kellnr registry，sdk/java/* 发布到私有 Maven 仓库；业务服务通过 `cargo add --registry kellnr` 或 `<dependency>` 引入；`Cargo.toml` 的 `members` 被过滤掉 `libs/*` 和 `sdk/rust/*` 行以避免指向不存在的目录。compose（services 与主入口）已统一转成 `image:` 引用且离线包含 `micro-server/frontend:<ver>` 镜像，启动无需现场构建前端；`frontend/` 源码保留在 airgap 骨架里供二次开发改完前端后用源码方式重打镜像。
- **镜像 tag no-v**：git tag 带 `v`（v1.5.0），发布镜像 tag 不带 v（`1.5.0`）；以下 `IMAGE_TAG` / 下游升级引用的都是 no-v 值。
- skills 稳定成 `.agents/skills/`（保留点开头，release.js 不再去点；agent 默认 discovery 路径即此）。

## 升级

```bash
# 在线升级
make upgrade TAG=v1.1.0

# 离线升级：先加载新镜像 tar，再运行
make upgrade TAG=v1.1.0
```

## 数据安全

- **不要提交** `secrets/`、`images/*.tar`、`.env` 到 git
- `jwt_private.pem` 权限必须为 `600`
- 生产环境建议将密钥存入 HSM/KMS
- 密码建议 90 天轮换一次
