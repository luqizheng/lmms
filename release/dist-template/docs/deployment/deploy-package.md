# 部署包（纯部署）发布指南

本文档说明如何从开发仓库生成**纯部署分发包**（`-deploy.tar.gz`）：不含源码 / AI 开发工具 / CI 脚本，仅有运行时 compose、镜像、精简运维脚本与配置，供直接私有化部署 / 上线。

> 打包统一由 Node 脚本完成（早期 shell 版 `release.sh` / `deploy.sh` 及 `start-deploy.sh` 已迁 JS 并以 `.legacy` 废弃保留在 `scripts/`、`bin/`、`Makefile.deploy.legacy`）。

## 两种分发包（各自由 dev 仓库独立产出）

| 包类型 | 产出脚本 | 内容 | 适用场景 | 产物 |
|--------|----------|------|----------|------|
| **airgap**（离+开发一体） | `node release/dist-template/scripts/release.js <ver>` | compose + 镜像 + 开发骨架（frontend 源码 / .agents/ skills / services 空骨架 …） | 二次开发 | `release/coder-micro-services-<ver>-airgap.tar.gz` |
| **deploy**（纯部署） | `node release/dist-template/scripts/release-deploy.js <ver>` | compose（已 image:）+ 镜像 + 部署运维 bin + Makefile + .env.example | 直接部署上线 | `release/coder-micro-services-<ver>-deploy.tar.gz` |

> **镜像 tag 一律 no-v**：git tag 用 `v1.0.0`，打包传入/引用镜像 tag 用 `1.0.0`（registry 里不存 `:v1.0.0`）。

```

  release.js（dev 仓库）         release-deploy.js（dev 仓库）
           │                              │
           ▼                              ▼
  coder-micro-services-      coder-micro-services-
  1.0.0-airgap.tar.gz         1.0.0-deploy.tar.gz
  （含开发骨架）              （纯运行时）
```

## 打包 deploy 包

在开发仓库根目录、有网（或无镜像时可用 `--no-images`）执行：

```bash
node release/dist-template/scripts/release-deploy.js 1.0.0
# 只打 compose/config/bin、跳过本地 build+save 镜像：
EXPORT_IMAGES=0 node release/dist-template/scripts/release-deploy.js 1.0.0
```

脚本阶段：
1. `EXPORT_IMAGES=1` 时调 `scripts/build-docker.js -v <ver> --no-push` 构建并加载全部镜像到本地；
2. 拷贝 `infrastructure/` 并跑 `scripts/transform-compose.js`——把 services.yml 与主 `docker-compose.yml` 中 `build:`（含 frontend）统一转成 `image:` 引用（部署包不带 frontend 源码，无需现场构建）；
3. `EXPORT_IMAGES=1` 时扫描所有 compose 的 `image:`，把未导出的 `docker save` 成 tar；
4. 写入 deploy-only 运维子集（`bin/` 白名单 + deploy 专用 `Makefile` + `docs/deployment/` + `.env.example` + `secrets/`）并**内嵌发布脚本本体**（`release-deploy.js` / `transform-compose.js` / `release-transforms.js`）；
5. 写 `MANIFEST.json`（`target=deploy`）并 `tar -czf` 打 `coder-micro-services-${ver}-deploy.tar.gz`。

## 部署

```bash
# 目标机器
tar -xzf coder-micro-services-1.0.0-deploy.tar.gz
cd coder-micro-services-1.0.0

make load-images     # = node bin/load-images.js
make secrets         # = node bin/generate-secrets.js（生成 .env + JWT 密钥）
make up              # = node bin/start.js --deploy（compose 引用镜像，无需本地构建）
make verify          # 冒烟
```

## 部署包内容（deploy-only）

```
coder-micro-services-1.0.0-deploy/
├── infrastructure/
│   ├── docker-compose*.yml            # 均已 image: 化（frontend 引用 micro-server/frontend:<ver>）
│   ├── config/                         # 运行时配置（<slug> 集中）
│   └── …（compose include 的基础设施/配置）
├── images/                             # Docker 镜像 tar（EXPORT_IMAGES=1 时）
├── bin/                                # 精简运维（.js，白名单）：
│   ├── start.js –deploy                # 启动（跳过前端源码构建）
│   ├── stop.js / status.js / logs.js / verify.js
│   ├── load-images.js / upgrade.js / generate-secrets.js
├── Makefile                            # deploy 专用（help/secrets/up/…）
├── .env.example
├── docs/deployment/                    # 部署相关文档
├── secrets/                            # 空目录，make secrets 填充
├── MANIFEST.json                       # target=deploy
└── release-deploy.js / transform-compose.js / release-transforms.js   # 内嵌，供再次生成/校验
```

| 环境变量 | 说明 |
|----------|------|
| `REGISTRY` | 镜像仓库，默认 `zhcoder-docker-registry.com:8000` |
| `IMAGE_TAG` | 引用/升级用**无 v** 镜像 tag（`.env` 里配置） |
| `EXPORT_IMAGES` | `0` 跳过本地 build+save（仅打 compose/config/bin） |

## 部署包不包含

| 内容 | 原因 |
|------|------|
| `frontend/` 源码 | 前端已预构建为 `micro-server/frontend:<ver>` 镜像并 image: 化 |
| `services/` | 服务源码用镜像 |
| `contracts/` / `scripts/`（CI 门禁） | 仅开发时需要 |
| `.agents/`（skills / rules） | AI 开发工具仅开发时需要 |
| `.cargo/` / `Cargo.toml` / `pnpm-workspace.yaml` / `package.json` / `AGENTS.md` | 开发 workspace / 协作文档 |

## 常见问题

### 新增业务服务镜像怎么进 deploy 包？

部署包不含 custom 镜像构建；请在开发仓库把新服务 `image:` 加进业务 compose，重跑 `release-deploy.js`（`EXPORT_IMAGES=1` 扫描所有 compose 的 `image:` 并补齐导出）。要点：compose 中声明 `image:`（而非 `build:`）才会被自动导出。

### 本地自定义改动后如何只出新镜像不动全量包？

`EXPORT_IMAGES=0 node release/dist-template/scripts/release-deploy.js <ver>` 只重建 compose/config 骨架；镜像 tar 需镜像导出步骤或补充 load。

### 部署包能用于二次开发吗？

不能——不含源码 / skills / contracts / Cargo.toml。需要二次开发用 **airgap** 包（`release.js`），并保留 `.agents/skills/`（见 ai-context / district docs）。
