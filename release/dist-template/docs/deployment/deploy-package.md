# 部署包发布指南

本文档说明如何从开发包中生成**纯部署包**——即去除源码、仅保留运行时所需文件的最小安装包。

## 两种分发包

| 包类型 | 产出脚本 | 内容 | 适用场景 |
|--------|---------|------|---------|
| **airgap**（离线开发包） | `scripts/release.sh` | 源码 + 镜像 + 开发骨架 | 二次开发 |
| **deploy**（纯部署包） | `scripts/deploy.sh` | compose + config + 镜像 | 直接部署 |

```
release.sh（在开发仓库）               deploy.sh（在分发包内）
         │                                      │
         ▼                                      ▼
  coder-micro-services               coder-micro-services
  -v1.0.0-airgap.tar.gz              -v1.0.0-deploy.tar.gz
  ┌─────────────────────┐            ┌──────────────────────┐
  │ frontend/ 源码       │            │ infrastructure/      │
  │ services/ 骨架       │            │   docker-compose*.yml│
  │ skills/ AI 开发工具   │            │   config/           │
  │ contracts/ 契约层    │            │   mysql-init/       │
  │ scripts/ CI 门禁     │            │ images/             │
  │ Cargo.toml           │            │ bin/（精简运维脚本）  │
  │ ...                  │            │ .env.example        │
  │                       │            │ Makefile            │
  │ + infrastructure/    │            │ docs/               │
  │ + images/            │            └──────────────────────┘
  └─────────────────────┘
  "拿回去改代码再构建"                  "拿回去直接部署上线"
```

## 完整工作流

### 第一步：打包开发包

在开发仓库执行：

```bash
cd release/dist-template
bash scripts/release.sh v1.0.0
```

产出 `release/coder-micro-services-v1.0.0-airgap.tar.gz`。

### 第二步：二次开发

解压开发包 → 修改前端源码 → 创建业务服务 → 调整 compose → 构建新镜像。

```bash
tar -xzf coder-micro-services-v1.0.0-airgap.tar.gz
cd coder-micro-services-v1.0.0

# 修改 frontend/ 源码
# 用 skills/ 创建业务服务
# 调整 infrastructure/docker-compose-services.yml

# 构建镜像
docker compose -f infrastructure/docker-compose.yml build
```

### 第三步：生成部署包

在分发包根目录执行 `deploy.sh`：

```bash
bash scripts/deploy.sh v1.0.0
```

脚本自动完成：

1. **复制 compose** — 把所有 `docker-compose*.yml` + `config/` 等基础设施文件复制到部署目录
2. **替换 frontend build** — 主 `docker-compose.yml` 中 frontend 的 `build:` 块替换为 `image:` 引用（因为部署包不含 `frontend/` 源码目录）
3. **同步镜像** — 扫描所有 compose 中的 `image:` 引用，对 `images/` 中缺失的镜像执行 `docker save`（二次开发中新构建的镜像会自动导出）
4. **打包** — 生成 `release/coder-micro-services-v${VERSION}-deploy.tar.gz`

### 第四步：部署

```bash
# 在目标机器上
tar -xzf coder-micro-services-v1.0.0-deploy.tar.gz
cd coder-micro-services-v1.0.0-deploy

make load-images     # 加载所有 Docker 镜像
make secrets         # 生成密钥和 .env
make up              # 启动所有服务
make verify          # 验证
```

## 命令参考

```bash
bash scripts/deploy.sh [VERSION] [--help]
```

| 参数 | 说明 |
|------|------|
| `VERSION` | 版本号，默认从 `.env` 读取 `IMAGE_TAG`，兜底 `v1.0.0` |
| `--help`, `-h` | 打印帮助 |

环境变量：

| 变量 | 说明 |
|------|------|
| `REGISTRY` | 镜像仓库，默认从 `.env` 读取或 `zhcoder-docker-registry.com:8000` |
| `IMAGE_TAG` | 镜像 tag（`.env` 中配置，`deploy.sh` 从 `.env` 读取） |

## 部署包内容

```
coder-micro-services-v1.0.0-deploy/
├── infrastructure/
│   ├── docker-compose.yml              # 主入口（frontend image: 引用）
│   ├── docker-compose-infra.yml        # 基础设施
│   ├── docker-compose-services.yml     # 业务服务（image: 引用）
│   ├── docker-compose-observability.yml # 可观测性
│   ├── docker-compose-infra.dev.yml    # 开发 overlay
│   ├── config/                         # 运行时配置
│   ├── mysql-init/                     # 数据库初始化
│   ├── nginx/                          # Nginx 配置
│   └── vendor/                         # 离线资源
├── images/                             # Docker 镜像 tar
├── bin/
│   ├── generate-secrets.sh
│   ├── load-images.sh
│   ├── start-deploy.sh                 # 部署版启动（不构建前端）
│   ├── stop.sh
│   ├── status.sh
│   ├── logs.sh
│   └── verify.sh
├── .env.example
├── Makefile
├── README.md
├── docs/
└── secrets/                            # 空目录，make secrets 填充
```

## 部署包不包含

以下内容仅在开发包中存在，部署包中已移除：

| 内容 | 原因 |
|------|------|
| `frontend/` | 前端已预构建在镜像中（`registry/micro-server/frontend:${VERSION}`） |
| `services/` | 服务源码不需要（运行时用镜像） |
| `contracts/` | 契约层仅开发时需要 |
| `scripts/` | CI 门禁脚本仅开发时需要 |
| `skills/` / `.agents/` | AI 开发工具仅开发时需要 |
| `.cargo/` | Rust 工具链配置 |
| `Cargo.toml` | Rust workspace |
| `pnpm-workspace.yaml` / `package.json` | Node workspace |
| `AGENTS.md` / `CHANGELOG.md` | 项目文档 |

## 常见问题

### 如何知道哪些镜像是新增的？

`deploy.sh` 的镜像同步阶段会输出详细统计：

```
[deploy]   save registry/micro-server/frontend:v1.0.0 → ...tar
[deploy]   ✓ 镜像同步: 1 个新增, 18 个已存在, 0 个缺失
```

### 如何处理自定义业务服务的镜像？

`deploy.sh` 会扫描所有 compose 文件中的 `image:` 引用。只要你的业务服务在 compose 中声明了 `image:`（而非 `build:`），就会自动导出。

### 部署包能再用于二次开发吗？

不能。部署包不含源码、skills、Cargo.toml 等开发工具。如需二次开发，请使用 `airgap` 包。
