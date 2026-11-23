# Public Release Pipeline Design

日期：2026-11-23
状态：实施中（PR #8）
范围：GitHub Actions tag release、公开 crates.io / npmjs.org / Docker Hub 发布；不包含 Maven 发布。

## 1. 背景

仓库已有 `coder release images` 与 `coder release sdk` 实现，但发布配置仍指向内网 Docker registry、Kellnr 和 Verdaccio；现有自动发布 runbook 也描述的是旧 GitLab / 内网目标。仓库新增了 GitHub Secrets：`CRATES`、`DOCKER_USER_NAME`、`DOCKER_KEY`、`NPMPUBLISHKEY`。

## 2. 目标

1. 仅 `vX.Y.Z` tag 触发正式发布；普通 push / PR 不得触碰发布凭据或发布目标。
2. 发布前在 GitHub Actions 执行发布门禁；任一门禁失败，所有发布 job 不得执行。
3. Rust libs 与 Rust SDK 发布到 crates.io；npm SDK 发布到 https://registry.npmjs.org；服务镜像发布到 Docker Hub。
4. 使用最小权限：发布 job 只获得自己生态的 secret；GitHub OIDC 权限仅在需要时启用。
5. 每个生态独立 job，失败可单独重跑；已存在版本按幂等跳过，其他错误必须失败。
6. 保留现有独立版本策略：服务镜像版本来自根 Cargo workspace；Rust crate / npm 包版本来自各自 manifest。
7. 不自动发布 Java/Maven；当前没有对应公开目标和凭据。

## 3. 目标与命名

- crates.io：默认 registry（不使用 Kellnr alias）；发布既有 `libs/*` 与 `sdk/rust/*` 清单，按依赖拓扑排序。
- npm：`https://registry.npmjs.org/`；发布 contracts、http 和五个服务 SDK。暂不发布 `@digua/cli`（版本仍为 0.0.0，且不是当前 SDK 发布范围）。
- Docker Hub：每个服务一个 repository，镜像引用为 `<DOCKER_USER_NAME>/<service>:<version>`；PG 变体使用 `-pg` 后缀。Docker Hub 不沿用内网 registry 的多级路径。

## 4. 发布流程

1. Tag workflow 验证 tag 严格匹配 `vMAJOR.MINOR.PATCH`，且 tag 指向的 commit 属于 main 历史。
2. Validate job 执行 Node 检查、Rust fmt/clippy/tests、OpenAPI drift、数据库/服务/契约门禁及 SDK 构建与测试。
3. publish-crates：只在 validate 成功后登录 crates.io 并按顺序发布；发布前执行 dry-run/preflight。
4. publish-npm：只在 validate 成功后配置 npm token，构建并发布到 npmjs.org；使用公开 access。
5. publish-docker：只在 validate 成功后登录 Docker Hub，构建并推送服务镜像；推送完成后生成部署 .image.env 产物供审阅，不直接改写 main。
6. 汇总 job 输出发布清单、版本与失败生态；不自动移动/覆盖 tag。

## 5. 安全与失败处理

- 发布 secrets 不得出现在 PR、pull_request_target、fork workflow 或 validate job。
- 发布 job 使用 `permissions: contents: read`；仅需要创建 GitHub Release 时才授予 contents: write。
- tag 必须是精确 SemVer；禁止覆盖已有 tag。
- crates.io / npm 已存在同版本视为幂等跳过；网络、认证、元数据或构建错误必须失败。
- Docker 镜像 tag 使用 no-v 版本；不得发布 latest，除非另行批准。
- 不打印 token，不把 token 写入仓库文件或 artifacts。

## 6. 非目标

- 不自动创建 GitHub Release / changelog（后续独立 PR）。
- 不自动部署生产环境。
- 不发布 Java SDK / Maven。
- 不修改 Rust crate 或 npm 包版本号；版本升级由独立变更完成。
- 不删除历史内网配置，直到确认无其他部署/消费场景依赖；本 PR 只调整正式发布目标与显式 npm publishConfig。

## 7. 验收

- 普通 PR 与 main push 不触发任何发布步骤。
- 非法 tag / 非 main 历史 tag 被拒绝。
- validate 失败时所有 publish job skipped。
- crates.io dry-run、npm pack/build、Docker build plan 可在无凭据条件下验证。
- 三个发布 job 分别只读取各自的 secret。
- workflow YAML 与相关 Node 单测通过。
