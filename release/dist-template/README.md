# coder-micro-services v1.0.0 - 离线分发包

> **v2 架构**：本包采用「平台镜像 + 开发骨架」模式。8 个核心服务以 Docker 镜像发布（已推送仓库），`services/` 目录为空，用户通过 `skills/` 下的 6 个 skill 自助创建 Rust 或 Java 业务微服务。

## 目录结构

```
coder-micro-services/
├── bin/                    # 一键运维脚本
├── compose/                # Docker Compose 模块化配置
│   ├── infra.yml           # 基础设施（MySQL/Redis/Consul/RabbitMQ/Registrator）
│   ├── services.yml        # 8 个核心服务（image: 引用仓库镜像）+ frontend
│   └── observability.yml   # 可观测性（Prometheus/Grafana/Jaeger/OTel）
├── config/                 # 运行时配置（api-gateway routes / grafana / consul / ...）
├── init-mysql/             # MySQL 初始化 SQL
├── frontend/               # 前端源码（pnpm-lock.yaml，不含 node_modules）
├── images/                 # 离线 Docker 镜像包 + manifest.json
├── secrets/                # 运行时生成的密钥（不提交 git）
│
├── ── v2 开发骨架 ──
│
├── services/               # 空 - 用户在此创建业务微服务
├── skills/                 # 6 个 AI skill（3 Rust + 3 Java）
├── libs/                   # Rust 共享库（common-error / common-db / jwt-ctx / ...）
├── sdk/                    # Java SDK + Rust SDK（业务服务调用核心服务的客户端）
├── contracts/              # 契约层（error-codes / permissions / events / conventions）
├── scripts/                # CI 门禁脚本（check-service-standard / check-db-naming / ...）
├── Cargo.toml              # Rust workspace 根（members: libs + sdk/rust，services/ 动态加入）
├── pnpm-workspace.yaml     # pnpm workspace 配置
├── package.json            # 根调度脚本（CI 门禁 / 构建 / 测试命令）
├── AGENTS.md               # 项目协作指南（skill 引用的约定）
│
├── docker-compose.yml      # 主入口（include 三个 yml）
├── Makefile                # make 命令统一入口
└── .env.example            # 环境变量模板
```

## 快速开始

### 1. 启动平台（核心服务 + 基础设施 + 前端）

```bash
# 在线环境
make secrets && make up && make verify

# 离线环境
bash bin/load-images.sh && make secrets && make up && make verify
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
3. AI skill 自动追加 compose + 网关路由           # docker-compose + config/api-gateway/routes/
4. 业务服务接入 docker compose up                # docker compose -f docker-compose.yml up -d {name}
5. CI 门禁通过                                   # check-service-standard / check-db-naming / check-spec
```

## 镜像版本

所有核心服务镜像统一版本：`${IMAGE_TAG}`
Registry：`${REGISTRY}`

## 离线镜像打包

```bash
# 在有网络的环境执行（建议 16GB+ 磁盘）
bash scripts/release.sh

# 输出：
#   release/coder-micro-services-v1.0.0-airgap.tar.gz
#   release/coder-micro-services-v1.0.0/
```

### 选项

| 用法 | 行为 |
| --- | --- |
| `bash scripts/release.sh [VERSION]` | 默认：pull + save 全部镜像 + 同步开发骨架 |
| `bash scripts/release.sh v1.2.0 --no-images` | 跳过 docker pull/save，仅打文档/配置/骨架包 |
| `EXPORT_IMAGES=0 bash scripts/release.sh [VERSION]` | 等价 `--no-images`（环境变量风格） |
| `bash scripts/release.sh --images [VERSION]` | 显式导出镜像（与默认等价，便于 CI 声明） |
| `bash scripts/release.sh --help` | 打印完整用法 |

- `--no-images` 同时跳过 pull 和 save；离线安装时需先 `docker pull` 或重跑 `EXPORT_IMAGES=1 bash scripts/release.sh`。
- flag 优先级高于 `EXPORT_IMAGES` 环境变量；`REGISTRY` 环境变量可覆盖默认仓库。
- v2 新增：release.sh 自动从开发仓库同步 `libs/` / `sdk/` / `contracts/` / `skills/` / `scripts/` / `frontend/`（不含 `node_modules` / `dist` / 缓存）+ `services/`（空）+ `Cargo.toml` / `pnpm-workspace.yaml` / `package.json` / `AGENTS.md` 到发布包。`frontend/` 是 compose/services.yml 中 frontend 服务的 build context，离线镜像缺失时供本地 docker build 兜底。

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
