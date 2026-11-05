# 上游更新：包升级指南（cms-sync 已停用）

> 下游业务项目（如 `project-manager`）如何升级到 coder-micro-services 新版本。
> 关联：[`../user-manual/12-release-and-distribute.md`](../user-manual/12-release-and-distribute.md) §12.6 / §12.10、[`./docker-registry-config`](./docker-registry-config)（registry 源配置）。

---

## 0. cms-sync 已停用（2026-10-30）

历史上下游通过 `cms-sync`（`@digua/dist-tools`，managed / three-way-merge / skip 策略）把上游 air-gap 包的源码文件同步进下游仓库。**该机制已停用**：

- 上游不再随 tag 发布 `@digua/dist-tools`；`release/dist-tools` 已从框架仓库移除。
- 下游不再 merge 上游源码文件。**框架的全部更新通过发布到各 registry 的包传递**。
- 已发布的历史版本（npm 上的 `@digua/dist-tools@≤1.x`）仍可安装，但不会再有更新，请勿在新项目使用。

上游发版的交付物 = **镜像 + 各语言包**（同一个 release tag 下版本号一致）：

| 交付物 | registry | 下游消费方式 |
|---|---|---|
| Docker 镜像（8 服务 + docs-site） | `zhcoder-docker-registry.com:8000/micro-server/<svc>:<ver>` | compose 改 image tag → `pull && up -d` |
| Rust crates（17 个 `libs/*` + 7 个 `sdk/rust/*`） | kellnr `http://192.168.2.12:16444` | `cargo update -p <crate>` |
| Java SDK（`io.digua.*` / `com.digua.*`） | Maven `http://mvn.kudi.repos` | 升 `<dependency>` 版本 |
| npm TS 包（`@digua/contracts` + `@digua/http`） | npm `http://npm.kudi.repos` | `pnpm up @digua/contracts @digua/http` |

---

## 1. 标准升级流程

以升级到 `1.4.0` 为例：

### 1.1 镜像（部署环境）

```bash
# compose 使用的 .env 里把 IMAGE_TAG 改为 no-v 版本号（v1.4.0 → 1.4.0）
# 填 v1.4.0 会 pull 不到镜像（CI 推送时已剥 v）
docker compose --env-file .env -f infrastructure/docker-compose.yml pull
docker compose --env-file .env -f infrastructure/docker-compose.yml up -d
node bin/verify.js   # 端到端冒烟
```

### 1.2 Rust 业务服务（kellnr）

```bash
# .cargo/config.toml 已配置 kellnr registry（新项目由 airgap 包骨架自带）
cargo update -p digua-http-client
cargo update -p common-error        # 只升用到的 crate；libs/* 与 sdk/rust/* 同版本发布
cargo build
```

### 1.3 Java 业务服务（Maven）

```xml
<!-- pom.xml：把 io.digua.* / com.digua.* 的 <version> 升到 1.4.0 -->
```

```bash
mvn -U clean verify
```

### 1.4 前端 / Node 侧（npm）

```bash
pnpm up @digua/contracts@1.4.0 @digua/http@1.4.0
pnpm run build
```

### 1.5 验证

- 镜像环境：`node bin/verify.js`（或逐服务 curl `/health`）。
- 业务侧：跑各自的单测 / e2e。contracts TS 产物随 `@digua/contracts` 包更新，下游无需本地重新生成。

---

## 2. air-gap 分发包现在的用途

`coder-micro-services-<ver>-airgap.tar.gz` **只用于新环境首次初始化**（离线起一套部署 + 业务开发骨架）：

- compose + `bin/` 运维脚本 + 各服务 config；
- `packages/`（npm TS 包源码）+ `contracts/`（契约与 codegen）+ `services/` 空骨架 + `.agents/skills`（业务服务脚手架）；
- `.cargo/config.toml`（kellnr）等基础配置。

**已有环境的版本升级不使用 air-gap 包**，走 §1 的包升级。如果确需对照某个文件在两个版本间的变化（罕见，如 config 默认值），手动解包新旧两份 air-gap 包自行 diff——不再有工具自动化这件事。

---

## 3. 与旧行为的差异

| 维度 | 旧（cms-sync） | 新（包升级） |
|---|---|---|
| 上游脚本 / contracts / skills 更新 | three-way-merge 同步进下游仓库 | 不再同步；下游按需手动取用（脚本在 airgap 包里，契约在 npm 包里） |
| `contracts` TS 产物 | 下游仓库内文件 | `@digua/contracts` npm 包 |
| 服务间 HTTP / SDK | 下游仓库内 `sdk/` 源码 | Maven / kellnr 包 |
| 镜像 | compose 随 sync 更新 | 下游自持 compose，只改 image tag |
| `.upstream-sync.yaml` / `.upstream.lock` | 必需 | **废弃**，可从下游仓库删除 |

---

## 4. 常见问题

| 现象 | 原因 | 处理 |
|---|---|---|
| `cargo update` 找不到新版本 | kellnr 未同步该版本（CI `rust_publish` 失败） | 查 tag pipeline 的 `rust_publish` job；确认 `.cargo/config.toml` 指向 kellnr |
| `pnpm up` 拉不到 `@digua/contracts@<ver>` | 内网 npm registry 未有该版本（`publish_packages` 失败） | 查 tag pipeline；`npm view @digua/contracts versions --registry http://npm.kudi.repos` |
| 镜像 pull 404 | 用了带 `v` 的 tag | IMAGE_TAG 用 no-v 版本号（`1.4.0`，不是 `v1.4.0`） |
| Maven 构件 404 | `sdk_publish` 失败或版本没对齐 | 查 tag pipeline；Maven 版本同样是 no-v |

---

## 5. 相关文档

- [`../user-manual/12-release-and-distribute.md`](../user-manual/12-release-and-distribute.md) — 发布全流程（镜像 / Maven / crates / npm）
- [`./docker-registry-config`](./docker-registry-config) — Docker / Cargo / Maven / npm 包源配置
- [`./commit-convention`](./commit-convention) — 提交规范
