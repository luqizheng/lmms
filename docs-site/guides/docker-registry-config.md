# Docker Registry 来源配置

> `pnpm run setup` 时如何选择 Docker 镜像的拉取来源。
> 适用于：本地开发、CI、离线分发包消费方。

---

## 概述

项目用到两类 Docker 镜像，可能来自不同仓库：

| 类别 | 包含 | 默认来源 | 可选配置 |
|------|------|----------|----------|
| **基础镜像** | rabbitmq / redis / nginx / mysql / consul / grafana / prometheus / jaeger / otel / registrator | Docker Hub (`hub.docker.com`) | 指定 mirror registry |
| **Builder 镜像** | rust / java / maven / node / alpine / docker-cli 等编译环境 | `zhcoder-docker-registry.com:8000` | 替换为其他私有 registry |

`pnpm run setup` 会通过交互式向导 `scripts/setup-wizard.js` 询问这两类来源，写入 `infrastructure/.docker-registry.json`（gitignored 本地配置），后续 `images:sync` + `images:pull` 消费该配置。

---

## 交互式向导

跑 `pnpm run setup` 时，在 `pnpm install` 后会进入向导：

```
=== Docker Registry Setup Wizard ===

1. 基础镜像（rabbitmq / redis / nginx / mysql / consul / grafana 等）
   默认从 Docker Hub (hub.docker.com) 拉取。若环境无法访问 Docker Hub，
   输入镜像仓库地址（如 zhcoder-docker-registry.com:8000/official）作为 mirror。

  Base registry [留空=Docker Hub]: _

2. Builder 镜像（rust / java / maven / node 等编译环境）
   默认从 zhcoder-docker-registry.com:8000 拉取。切换到其他私有仓库时，
   输入 registry 地址（如 registry.my.com:5000）。

  Builder registry [zhcoder-docker-registry.com:8000]: _
```

### 两类镜像的处理方式

**基础镜像**（如 `redis:7-alpine`）：
- 留空 = 直接从 Docker Hub 拉原始 ref（`docker pull redis:7-alpine`）
- 输入 URL = 作为 mirror 前缀，`images-pull` 会先尝试 `docker pull <baseRegistry>/<ref>`（如 `zhcoder-docker-registry.com:8000/official/redis:7-alpine`），失败回退到原始 ref

**Builder 镜像**（如 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`）：
- 默认 = 直接拉原始 ref
- 输入其他 registry = **重写 ref 的 registry 前缀**（如 `registry.my.com:5000/builder/rust:1.96-slim-musl`），mirror 字段留空

---

## 非交互模式（CI / 自动化）

stdin 不是 TTY 时自动跳过交互，按以下优先级解析配置：

**优先级**（高 -> 低）：环境变量 > 配置文件 > 默认值

| 环境变量 | 作用 | 默认值 |
|----------|------|--------|
| `CODER_BASE_REGISTRY` | 基础镜像来源（空=docker hub） | 空 |
| `CODER_BUILDER_REGISTRY` | Builder 镜像仓库 | `zhcoder-docker-registry.com:8000` |

CI 示例：

```bash
# CI 用内网 mirror 拉基础镜像，builder 用默认
CODER_BASE_REGISTRY=zhcoder-docker-registry.com:8000/official pnpm run setup

# 或显式跳过向导（用默认值）
node scripts/setup-wizard.js --defaults

# 或指定两个 registry
CODER_BASE_REGISTRY=my-mirror.com/official \
CODER_BUILDER_REGISTRY=registry.my.com:5000 \
pnpm run setup
```

---

## 命令行参数

```bash
node scripts/setup-wizard.js                 # 交互式（默认，stdin 是 TTY 时）
node scripts/setup-wizard.js --non-interactive   # 跳过交互，用 env var 或配置文件
node scripts/setup-wizard.js --defaults          # 强制用默认值
node scripts/setup-wizard.js --show              # 打印当前配置后退出
node scripts/setup-wizard.js --help              # 打印帮助
```

也可以用 pnpm 别名：

```bash
pnpm run setup:wizard                           # = node scripts/setup-wizard.js
pnpm run setup:wizard -- --show                 # 传参给脚本
```

---

## 配置文件

`infrastructure/.docker-registry.json`（gitignored，不入 git）：

```json
{
  "version": 1,
  "baseRegistry": "",
  "builderRegistry": "zhcoder-docker-registry.com:8000",
  "updatedAt": "2026-07-21T06:37:12.517Z"
}
```

| 字段 | 含义 |
|------|------|
| `baseRegistry` | 空字符串 = Docker Hub；非空 = mirror registry URL |
| `builderRegistry` | Builder 镜像仓库，默认 `zhcoder-docker-registry.com:8000` |
| `updatedAt` | 最后更新时间（向导写入时自动填） |

文件不存在时，`base-images-sync.js` 回退到默认值（等价于 `--defaults`）。

---

## 配置如何生效

`pnpm run setup` 的完整流程：

```
pnpm run setup
  ├─ pnpm install
  ├─ node scripts/setup-wizard.js    # 写入 .docker-registry.json
  ├─ pnpm run images:sync            # 读取 .docker-registry.json，生成 base-images.json
  │                                   #   - 基础镜像: mirror 字段 = baseRegistry
  │                                   #   - Builder 镜像: ref 前缀重写为 builderRegistry
  ├─ pnpm run images:pull            # 按 base-images.json 拉镜像
  │                                   #   - 有 mirror: 先试 <mirror>/<ref>，失败回退 <ref>
  │                                   #   - 无 mirror: 直接拉 <ref>
  ├─ pnpm run assets:download        # 下载 swagger-ui zip
  └─ pnpm run check:env              # 环境体检
```

### base-images.json 示例

默认配置（base=docker hub, builder=zhcoder-docker-registry.com:8000）：

```json
{
  "images": [
    { "ref": "redis:7-alpine", "mirror": null },
    { "ref": "mysql:8.0", "mirror": null },
    { "ref": "zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl", "mirror": null }
  ]
}
```

自定义配置（base=`my-mirror.com/official`, builder=`registry.my.com:5000`）：

```json
{
  "images": [
    { "ref": "redis:7-alpine", "mirror": "my-mirror.com/official" },
    { "ref": "mysql:8.0", "mirror": "my-mirror.com/official" },
    { "ref": "registry.my.com:5000/builder/rust:1.96-slim-musl", "mirror": null }
  ]
}
```

---

## 离线分发包（dist 包）

`release/dist-template/scripts/release.sh` 把 `scripts/` 整个复制到分发包，包括 `setup-wizard.js` 和改造后的 `base-images-sync.js` / `Base-Images-Writer.js`。分发包用户解压后跑 `pnpm run setup` 同样会进入交互式向导。

详见 [12 发布与分发 § 12.6 消费方快速开始](../user-manual/12-release-and-distribute#12-6-消费方快速开始拿到-targz-后-4-步)。

---

## 常见场景

### 场景 1：内网环境，Docker Hub 不可达

```bash
$ node scripts/setup-wizard.js

  Base registry [留空=Docker Hub]: zhcoder-docker-registry.com:8000/official
  Builder registry [zhcoder-docker-registry.com:8000]:    # 回车用默认
```

效果：基础镜像从 `zhcoder-docker-registry.com:8000/official/redis:7-alpine` 拉，失败回退 `redis:7-alpine`。

### 场景 2：切换到其他私有 registry

```bash
$ node scripts/setup-wizard.js

  Base registry [留空=Docker Hub]: registry.my.com:5000/official
  Builder registry [zhcoder-docker-registry.com:8000]: registry.my.com:5000
```

效果：
- 基础镜像 mirror = `registry.my.com:5000/official`
- Builder 镜像 ref 重写为 `registry.my.com:5000/builder/rust:1.96-slim-musl`

### 场景 3：切回默认

```bash
$ node scripts/setup-wizard.js --defaults
```

效果：清空 baseRegistry，builderRegistry 还原为 `zhcoder-docker-registry.com:8000`。

### 场景 4：CI 自动化

```yaml
# .gitlab-ci.yml
test:
  script:
    - CODER_BASE_REGISTRY=zhcoder-docker-registry.com:8000/official pnpm run setup
    - pnpm test
```

---

## 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| `docker pull` 超时 | 基础镜像从 Docker Hub 拉，网络不通 | 重新跑 `node scripts/setup-wizard.js`，输入内网 mirror |
| Builder 镜像拉取失败 | `zhcoder-docker-registry.com:8000` 不可达 | 重新跑向导，输入可达的 registry |
| `base-images.json` 里 builder ref 还是旧的 | 配置文件未更新或 `images:sync` 未重跑 | 先 `node scripts/setup-wizard.js`，再 `pnpm run images:sync` |
| CI 报 `images:sync` drift | 本地配置和 CI 不一致 | CI 用 `CODER_BASE_REGISTRY` / `CODER_BUILDER_REGISTRY` 环境变量对齐 |
| 想看当前配置 | - | `node scripts/setup-wizard.js --show` |
| 向导不出现，直接跳过 | stdin 不是 TTY（CI 环境） | 正常行为，用 env var 或 `--defaults`；本地强制交互用 `--non-interactive=false`（待实现） |
| `.docker-registry.json` 被误提交 | - | 已在 `.gitignore` 忽略；若已提交，`git rm --cached infrastructure/.docker-registry.json` |

---

## 相关文件

| 文件 | 作用 |
|------|------|
| `scripts/setup-wizard.js` | 交互式向导，写入配置文件 |
| `scripts/base-images-sync.js` | 读取配置，生成 `base-images.json` |
| `scripts/lib/Base-Images-Writer.js` | `mergeMirror` 逻辑（builder ref 重写 + base mirror） |
| `scripts/images-pull.js` | 按 `base-images.json` 拉镜像（消费 mirror 字段） |
| `infrastructure/.docker-registry.json` | 本地配置文件（gitignored） |
| `infrastructure/base-images.json` | 生成的镜像清单（入 git） |

---

## 相关文档

- [新人入门 § 02 环境准备](../onboarding/02-环境准备)
- [用户手册 § 01 快速上手](../user-manual/01-quick-start)
- [用户手册 § 12 发布与分发](../user-manual/12-release-and-distribute)
- [infrastructure/README.md](https://github.com/coder-micro-services/low-memory-micro-services/blob/main/infrastructure/README.md) - Docker Compose 工作流
