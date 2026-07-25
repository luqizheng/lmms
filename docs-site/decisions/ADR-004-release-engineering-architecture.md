# ADR-004: Release Engineering Architecture

> 日期：2026-07-23
> 状态：已采纳（追认 7 月发行工程链路）
> 关联：
> - [release-dist-template spec](../superpowers/specs/2026-07-23-release-dist-template-design.md)
> - [setup-wizard spec](../superpowers/specs/2026-07-23-setup-wizard-design.md)
> - [SDK publishing spec](../superpowers/specs/2026-07-23-sdk-publishing-design.md)

## 上下文

7 月项目从"仅在本地/开发环境跑"转向"可交付给客户私有化部署"。带来 4 个新问题：

1. **离线部署**：客户环境无外网，需要 Docker 镜像 + 配置 + 部署脚本打成单包
2. **镜像/依赖源**：不同环境（个人机 / CI / 客户）走不同 registry mirror（rsproxy / 内网 / 镜像仓库）
3. **多语言 SDK 发布**：Rust libs + Java SDK 需要发到私有仓库供业务方消费
4. **v2 分发形态**：不再把 services 源码打包，而是"平台镜像 + 开发骨架"—— 用户拿骨架 + skills 自建业务微服务

ADR-002 定的是"单仓开发规范"，7 月这些工作打通"仓→客户"链路，需要单独 ADR 沉淀。

## 决议

### 1. 分发形态：v2「平台镜像 + 开发骨架」

**核心平台以 Docker 镜像交付**，`services/` 目录为空供用户自建业务服务。

```
coder-micro-services-<version>-airgap.tar.gz
├── bin/                # 一键运维脚本（load-images / make secrets / verify）
├── compose/            # docker-compose 模块化（infra + services + observability）
├── config/             # 运行时配置（gateway routes / grafana / consul / ...）
├── images/             # 离线镜像 tar + manifest.json
├── frontend/           # 前端源码 + pnpm-lock.yaml
├── services/           # 空 —— 用户在此创建业务微服务
├── skills/             # 6 个 AI skill（3 Rust + 3 Java）
├── contracts/          # 契约层
├── scripts/            # CI 门禁脚本
├── Cargo.toml          # workspace 根（libs 保留，services 通过 sed 过滤）
├── docker-compose.yml  # 主入口
├── Makefile            # 统一命令入口
└── .env.example
```

**放弃"服务源码全打包"路径**：太重、更新不便、客户无需读服务源码。

### 2. 打包工具：`release/dist-template/scripts/release.sh`

- 单一入口：`bash release/dist-template/scripts/release.sh v1.0.0`
- 可选 `--no-images`（`EXPORT_IMAGES=0`）：跳过 docker pull + save（骨架 only）
- 8 个核心服务镜像清单硬编码在脚本中
- 输出 `.tar.gz` 单文件 airgap 包

### 3. Registry 抽象：三重 fallback

用户 setup 时可配置 4 类 mirror（全部可留空走官方源）：

| 类别 | 官方源 | 替代 |
|------|--------|------|
| Docker 基础镜像 | Docker Hub | Nexus / Harbor（`baseRegistry`）|
| Docker builder 镜像 | 官方 | 内网 registry（默认 `zhcoder-docker-registry.com:8000`）|
| Cargo crates | crates.io | rsproxy / kellnr 私有 registry |
| Maven | Maven Central | Nexus / Reposilite（`token` / `userpass` 认证）|
| npm | npmjs | 镜像仓库 |

`setup-wizard` 交互式收集 → 写入 `.cargo/config.toml` / `.mvn/settings.xml` / `.npmrc` / `infrastructure/.docker-registry.json`（全部 gitignored）。

### 4. SDK 发布策略：私有仓库

| SDK | 目标仓库 | 认证 |
|-----|---------|------|
| Rust libs（`libs/`）+ SDK（`sdk/rust/`）| 私有 kellnr | `.cargo/credentials.toml` 中 token |
| Java SDK（`sdk/java/`）| 私有 Reposilite (`192.168.2.12:18181`) | `.mvn/settings.xml` 中 servers |
| .NET SDK | ❌ **已废弃**（7 月清理）| — |

**Java SDK 版本策略**：`0.3.0` 起独立版本号，与平台版本解耦（SDK 消费方按需锁定）。

### 5. Setup wizard 是"零信任本地化配置"

`scripts/setup-wizard.js`：
- 首次 clone 后跑一次
- 全部输出文件**不入 git**（个人/团队/CI 隔离）
- 支持 `--non-interactive` / `--defaults` / `--show` / `--dry-run`
- 同步部分选项到 `.gitlab-ci.yml`（CODER_* 变量）

## 后果

### 好处

- **一次打包多环境部署**：同一 airgap 包可在客户/内网/CI 部署
- **依赖源可插拔**：不同环境走不同 mirror，代码零改动
- **v2 骨架模式减少发行体积**：镜像 10-15 GB，骨架仅 MB 级
- **AI skill 自建服务**：客户可用 skills 快速新增业务微服务，无需读平台源码

### 代价

- `release/dist-template/` 是"历史 release archive"目录，包含 v1.0.0 的完整快照 —— 未来会积累多版本，需清理策略
- `setup-wizard.js` 891 行，5 类 registry 的交互复杂度高
- `.cargo/config.toml` 被 wizard 覆盖式重写 —— 用户手改的其他配置会丢
- SDK 版本号与平台版本解耦 —— 消费方需自行匹配兼容性
- `sdk/rust/` + `libs/` 发布到私有 kellnr —— 若 kellnr 挂了阻塞发布

### 已知未做

- **release archive 清理策略**：现存 `release/coder-micro-services-1.0.0/` 1795 files，未来 v2.0.0 后如何清理老版本？
- **airgap 包完整性校验**：无 checksum / GPG 签名
- **SDK 语义化版本**：目前仅 Java 用了 `0.3.0`，Rust 版本策略未固化
- **多架构镜像**：目前只 `linux/amd64`；`docker:build` 支持 `--platform linux/amd64,linux/arm64` 但未在 release.sh 里默认

## 变更时机

- **新服务发布**：加入 `release.sh` 镜像清单 + `compose/services.yml` + `openapi.yaml` 打包
- **新依赖源**：`setup-wizard.js` 加交互 + 生成对应配置文件（模式参考现有 5 类）
- **SDK 破坏性变更**：升 major 版本 + 在 `sdk/README.md` 记录 migration guide
- **v2 骨架结构调整**：改 `release.sh` 复制清单 + 更新 `release/dist-template/README.md`
