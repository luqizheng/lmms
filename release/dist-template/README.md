# coder-micro-services v1.0.0 — 离线分发包

## 目录结构

```
coder-micro-services/
├── bin/                    # 一键运维脚本
│   ├── generate-secrets.sh # 生成 JWT 密钥 + 随机密码
│   ├── start.sh            # 启动所有容器
│   ├── stop.sh             # 停止容器（保留数据卷）
│   ├── status.sh           # 查看容器状态
│   ├── logs.sh             # 查看日志
│   ├── verify.sh           # 端到端冒烟测试
│   ├── reset-data.sh       # 清除数据并重置
│   ├── seed-demo.sh        # 写入 demo 数据
│   ├── build-frontend.sh   # 构建前端 nginx 镜像
│   ├── load-images.sh      # 从离线 tar 加载镜像
│   └── upgrade.sh          # 原地升级
├── compose/                # Docker Compose 模块化配置
│   ├── infra.yml           # 基础设施（MySQL/Redis/Consul/RabbitMQ/Registrator）
│   ├── services.yml        # 8 个 Rust 微服务 + frontend
│   └── observability.yml    # 可观测性（Prometheus/Grafana/Jaeger/OTel）
├── config/                 # 25 个配置文件
├── init-mysql/             # MySQL 初始化 SQL
├── frontend/               # 前端源码（pnpm-lock.yaml，不含 node_modules）
├── images/                 # 离线 Docker 镜像包 + manifest.json
├── secrets/                # 运行时生成的密钥（不提交 git）
├── docker-compose.yml      # 主入口（include 三个 yml）
├── Makefile               # make 命令统一入口
└── .env.example           # 环境变量模板
```

## 快速开始

### 在线环境（能访问 registry）

```bash
# 1. 初始化 .env（自动生成密钥）
make secrets

# 2. 构建前端镜像并启动
make up

# 3. 验证
make verify
```

### 离线环境（air-gap）

```bash
# 1. 加载离线镜像（交付时由运维提供 images/*.tar）
bash bin/load-images.sh

# 2. 初始化 .env
make secrets

# 3. 启动
make up

# 4. 验证
make verify
```

## 访问地址

| 服务 | 地址 | 默认账号 |
|------|------|---------|
| 前端 | http://localhost:3000 | — |
| API 网关 | http://localhost:8080 | — |
| Consul | http://localhost:8500 | — |
| Prometheus | http://localhost:9090 | — |
| Grafana | http://localhost:3001 | admin / admin |
| Jaeger | http://localhost:16686 | — |
| RabbitMQ | http://localhost:15672 | guest / guest |

## 服务清单（8 微服务 + 前端）

| 服务 | 端口 | 说明 |
|------|------|------|
| api-gateway | 8080 | Kong/nghttp2 网关 |
| config-service | 8081 | 配置中心 |
| file-service | 8082 | 文件管理 |
| token-service | 8083 | Token 管理（outbox pattern）|
| iam-service | 8084 | 身份与访问管理 |
| notification-service | 8085 | 通知服务（outbox pattern）|
| audit-service | 8086 | 审计日志 |
| prue-backend | 8087 | Prueba 后端（MySQL+SQLite）|
| frontend | 3000 | Vue 3 前端（nginx）|

## 镜像版本

所有微服务镜像统一版本：`v1.0.0`
Registry：`zhcoder-docker-registry.com:8000`

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
| `bash scripts/release.sh [VERSION]` | 默认：pull + save 全部镜像 |
| `bash scripts/release.sh v1.2.0 --no-images` | 跳过 docker pull/save，仅打文档/配置包 |
| `EXPORT_IMAGES=0 bash scripts/release.sh [VERSION]` | 等价 `--no-images`（环境变量风格） |
| `bash scripts/release.sh --images [VERSION]` | 显式导出镜像（与默认等价，便于 CI 声明） |
| `bash scripts/release.sh --help` | 打印完整用法 |

- `--no-images` 同时跳过 pull 和 save；离线安装时需先 `docker pull` 或重跑 `EXPORT_IMAGES=1 bash scripts/release.sh`。
- flag 优先级高于 `EXPORT_IMAGES` 环境变量；`REGISTRY` 环境变量可覆盖默认仓库。

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